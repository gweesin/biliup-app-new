use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::Manager;
use std::process::Stdio;
use tokio::process::Command as TokioCommand;
use tracing::{error, info, warn};

use crate::error::AppError;
use crate::utils::crypto::encode_base64;
use crate::{AppData, models::AiConfig};

/// 提交给 AI 的提示词：切入角度与表达风格完全交给模型自主决定，不预设固定模板
const AI_PROMPT: &str = "请处理这张 MOBA 游戏梦三国2的对局结算截图，完成信息提取，并自由创作一个战报标题。

第一步 提取信息
- 对局胜负结果与双方阵营的最终比分
- 绿底高亮行对应的英雄名称（只取英雄名；英雄名多为三国人物名，带“梦”前缀时写作“梦许褚”这种形式，不要带玩家名）
- 该英雄的 KDA（击杀 / 死亡 / 助攻）

第二步 自由创作标题
围绕英雄名创作 1 个最有冲击力、最适合游戏高光展示的标题：
- 切入角度、表达风格、句式结构、英雄名所在位置，全部由你自己决定，不要套用任何固定模板，也不要沿用你的第一反应句式
- 先在脑中快速构思 5 个方向完全不同的标题（不同角度、不同语气、不同长度、不同修辞），再从中挑出最新鲜、最有画面感、最不像模板的那一个
- 标题中必须出现英雄名，长度 8-20 字，简短有力，符合游戏社区的表达习惯，可结合当下热点话题吸引流量
- 避免“秀翻全场”“碾压全场”“无人能敌”这类被用烂的空泛套话，也避免与常见游戏视频标题雷同
- 允许口语、玩梗、夸张、古风、悬念、反差、第一人称等任意风格，只要不低俗、不误导
- 可以结合对局结果、最终比分、英雄名、KDA 等信息

只输出最终那 1 个标题文本，不要编号、不要引号、不要列表、不要任何解释或前后缀文字。";

/// 截取视频（倒数第三秒）画面并请求 AI 生成一个标题
#[tauri::command]
pub async fn generate_ai_title(
    app: tauri::AppHandle,
    video_path: String,
) -> Result<String, AppError> {
    let video_path = video_path.trim().to_string();
    if video_path.is_empty() || !Path::new(&video_path).is_file() {
        return Err(AppError::Custom("视频文件不存在或路径无效".to_string()));
    }

    let ai = app.state::<AppData>().config.lock().await.ai.clone();

    check_ai_config(&ai)?;

    // 1. 解析 ffmpeg 可执行文件
    let ffmpeg = resolve_ffmpeg(&ai.ffmpeg_path).ok_or_else(|| {
        AppError::Custom(
            "未找到 ffmpeg，无法截取视频画面。请安装 ffmpeg 并加入系统 PATH，\
             或在「全局设置 → AI 设置」中填写 ffmpeg.exe 的完整路径。\
             可参考下载地址: https://www.gyan.dev/ffmpeg/builds/"
                .to_string(),
        )
    })?;
    info!("使用 ffmpeg: {}", ffmpeg.display());

    // 2. 探测视频总时长（秒）
    let duration = probe_video_duration(&ffmpeg, &video_path).await?;
    info!("视频时长: {duration:.3}s, 路径: {video_path}");

    // 3. 截取倒数第三秒画面
    let target = (duration - 3.0).max(0.0);
    let frame_bytes = extract_frame(&ffmpeg, &video_path, target).await?;
    if frame_bytes.is_empty() {
        return Err(AppError::Custom("截取视频画面失败，输出为空".to_string()));
    }
    let data_url = format!("data:image/jpeg;base64,{}", encode_base64(&frame_bytes));
    info!("视频画面截取成功, {} 字节, 时间点 {target:.3}s", frame_bytes.len());

    // 4. 请求 OpenAI 兼容的视觉接口，取 AI 生成的单个标题
    let title = request_ai_title(&ai, data_url).await?;
    Ok(title)
}

/// 校验 AI 配置是否完整可用
fn check_ai_config(ai: &AiConfig) -> Result<(), AppError> {
    if !ai.enabled {
        return Err(AppError::Custom(
            "AI 标题生成尚未启用，请先在「全局设置 → AI 设置」中开启并填写配置".to_string(),
        ));
    }
    if ai.base_url.trim().is_empty() {
        return Err(AppError::Custom(
            "尚未配置 AI 接口地址，请先在「全局设置 → AI 设置」中填写 Base URL".to_string(),
        ));
    }
    if ai.api_key.trim().is_empty() {
        return Err(AppError::Custom(
            "尚未配置 AI 接口密钥，请先在「全局设置 → AI 设置」中填写 API Key".to_string(),
        ));
    }
    if ai.model.trim().is_empty() {
        return Err(AppError::Custom(
            "尚未配置 AI 模型名称，请先在「全局设置 → AI 设置」中填写模型".to_string(),
        ));
    }
    Ok(())
}

/// 查找 ffmpeg 可执行文件：优先使用配置路径，其次搜索 PATH 与常见安装目录
fn resolve_ffmpeg(configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
        warn!("配置的 ffmpeg 路径无效，尝试自动搜索: {configured}");
    }

    let exe_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };

    // 搜索系统 PATH
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // Windows 常见安装位置
    #[cfg(windows)]
    {
        let mut candidates = vec![
            PathBuf::from(r"C:\ffmpeg\bin").join(exe_name),
            PathBuf::from(r"D:\ffmpeg\bin").join(exe_name),
            PathBuf::from(r"C:\Program Files\ffmpeg\bin").join(exe_name),
            PathBuf::from(r"C:\Program Files (x86)\ffmpeg\bin").join(exe_name),
        ];
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            // Scoop / Chocolatey 等包管理器的常见安装位置
            candidates.push(PathBuf::from(&user_profile).join("scoop/shims").join(exe_name));
            candidates.push(
                PathBuf::from(&user_profile)
                    .join("scoop/apps/ffmpeg/current/bin")
                    .join(exe_name),
            );
            candidates.push(
                PathBuf::from(&user_profile)
                    .join("AppData/Local/Microsoft/WinGet/Links")
                    .join(exe_name),
            );
        }
        for candidate in candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // macOS / Linux 常见安装位置
    #[cfg(not(windows))]
    {
        for candidate in [
            "/usr/bin/ffmpeg",
            "/usr/local/bin/ffmpeg",
            "/opt/homebrew/bin/ffmpeg",
            "/snap/bin/ffmpeg",
        ] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

/// 执行外部进程并捕获 stdout / stderr
async fn run_process(
    program: &Path,
    args: &[&str],
    timeout_secs: u64,
) -> Result<(Vec<u8>, String), AppError> {
    let mut cmd = TokioCommand::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| AppError::Custom(format!("无法启动 {}: {e}", program.display())))?;

    let output = tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output())
        .await
        .map_err(|_| AppError::Custom(format!("{} 执行超时", program.display())))?
        .map_err(|e| AppError::Custom(format!("执行 {} 失败: {e}", program.display())))?;

    Ok((output.stdout, String::from_utf8_lossy(&output.stderr).into_owned()))
}

/// 探测视频时长（秒）。优先使用同目录 ffprobe，失败时解析 ffmpeg -i 输出
async fn probe_video_duration(ffmpeg: &Path, video_path: &str) -> Result<f64, AppError> {
    // 优先使用 ffprobe（通常与 ffmpeg 同目录安装）
    let ffprobe = ffmpeg.with_file_name(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" });
    if ffprobe.is_file() {
        let (stdout, stderr) = run_process(
            &ffprobe,
            &[
                "-v", "error", "-show_entries", "format=duration", "-of",
                "default=noprint_wrappers=1:nokey=1", video_path,
            ],
            30,
        )
        .await?;
        let text = String::from_utf8_lossy(&stdout).trim().to_string();
        if let Ok(secs) = text.parse::<f64>() {
            return Ok(secs);
        }
        warn!("ffprobe 解析时长失败: {text:?} | {stderr}");
    }

    // 回退：解析 ffmpeg -i 输出的 Duration 字段
    let (_stdout, stderr) = run_process(ffmpeg, &["-hide_banner", "-i", video_path], 30).await?;
    if let Some(secs) = parse_ffmpeg_duration(&stderr) {
        return Ok(secs);
    }

    Err(AppError::Custom(
        "无法获取视频时长（视频文件可能损坏或编码不受支持）。".to_string(),
    ))
}

/// 从 ffmpeg -i 的 stderr 输出中解析 Duration: HH:MM:SS.xx
fn parse_ffmpeg_duration(stderr: &str) -> Option<f64> {
    const MARKER: &str = "Duration: ";
    let pos = stderr.find(MARKER)?;
    let rest = &stderr[pos + MARKER.len()..];
    let token = rest.split(',').next().unwrap_or("").trim();
    if token.is_empty() || token.eq_ignore_ascii_case("N/A") {
        return None;
    }

    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: f64 = parts[0].trim().parse().ok()?;
    let minutes: f64 = parts[1].trim().parse().ok()?;
    let seconds: f64 = parts[2].trim().parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// 截取指定时间点的视频帧，返回 JPEG 图片字节
async fn extract_frame(
    ffmpeg: &Path,
    video_path: &str,
    at_second: f64,
) -> Result<Vec<u8>, AppError> {
    let seek_arg = format!("{at_second:.3}");
    let (stdout, stderr) = run_process(
        ffmpeg,
        &[
            "-y",
            "-ss",
            &seek_arg,
            "-i",
            video_path,
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-c:v",
            "mjpeg",
            "-q:v",
            "5",
            "pipe:1",
        ],
        60,
    )
    .await?;

    if stdout.is_empty() {
        return Err(AppError::Custom(format!(
            "截取视频画面失败: {}",
            stderr.lines().next_back().unwrap_or("未知错误")
        )));
    }
    Ok(stdout)
}

/// 拼接 OpenAI 兼容的 chat/completions 接口地址
fn build_chat_endpoint(base_url: &str) -> String {
    let mut base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return String::new();
    }
    // 兼容直接填写完整接口地址的情况
    if base.ends_with("/chat/completions") {
        return base;
    }
    // 未带协议时补充 https://
    if !base.contains("://") {
        base = format!("https://{base}");
    }
    format!("{base}/chat/completions")
}

/// 请求 OpenAI 兼容视觉接口，返回 AI 生成的单个标题
async fn request_ai_title(ai: &AiConfig, image_data_url: String) -> Result<String, AppError> {
    let endpoint = build_chat_endpoint(&ai.base_url);
    if endpoint.is_empty() {
        return Err(AppError::Custom("AI 接口地址无效".to_string()));
    }

    let mut body = json!({
        "model": ai.model.trim(),
        // 创意写作区间，让模型自行发散：越高越跳脱，越低越稳定
        "temperature": 1.2,
        // 显式关闭流式，避免个别端点默认返回 SSE
        "stream": false,
        "messages": [
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": AI_PROMPT },
                    {
                        "type": "image_url",
                        "image_url": { "url": image_data_url }
                    }
                ]
            }
        ]
    });

    // 思考模式（DeepSeek 等接口）：显式下发 thinking 参数开启。
    // 图片提取类任务推理点有限：默认 reasoning_effort=high 会把大量输出预算花在
    // 与任务无关的思考上（如分析图片格式），导致正文长度不足甚至为空。
    // 因此默认用 low 缩短思考，把预算留给正文；同时也放宽 max_tokens 兜底。
    // 注意：这些参数非 OpenAI 标准，不支持的接口请在「全局设置 → AI 设置」中关闭思考模式
    if ai.thinking {
        let effort = normalize_reasoning_effort(&ai.reasoning_effort);
        body["thinking"] = json!({ "type": "enabled" });
        body["reasoning_effort"] = json!(effort);
        body["max_tokens"] = json!(2000);
        info!("AI 请求已开启思考模式 (thinking=enabled, reasoning_effort={effort}, max_tokens=2000)");
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| AppError::Custom(format!("创建 HTTP 客户端失败: {e}")))?;

    let mut request = client.post(&endpoint).json(&body);
    let api_key = ai.api_key.trim();
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }

    info!("请求 AI 接口: {endpoint}, 模型: {}", ai.model.trim());
    let response = match request.send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("请求 AI 接口失败: {e}");
            return Err(AppError::Custom(format!("请求 AI 接口失败: {e}")));
        }
    };

    let status = response.status();
    let text = match response.text().await {
        Ok(t) => t,
        Err(e) => return Err(AppError::Custom(format!("读取 AI 响应失败: {e}"))),
    };

    if !status.is_success() {
        let brief: String = text.chars().take(500).collect();
        return Err(AppError::Custom(format!(
            "AI 接口返回错误 (HTTP {status}): {brief}"
        )));
    }

    // 记录响应片段，便于排查（截断避免日志过大）
    info!(
        "AI 响应 (HTTP {status}): {}",
        text.chars().take(1000).collect::<String>()
    );

    let parsed: Value = serde_json::from_str(&text).map_err(|e| {
        AppError::Custom(format!(
            "解析 AI 响应失败: {e}; 原始响应: {}",
            text.chars().take(500).collect::<String>()
        ))
    })?;

    let (content_text, reasoning_text) = extract_message_content(&parsed);
    if content_text.trim().is_empty() {
        let brief: String = text.chars().take(800).collect();
        if reasoning_text.is_some() {
            return Err(AppError::Custom(format!(
                "AI 只返回了思考内容（reasoning）而没有正文，通常是思考模式耗尽了输出长度。\
                 请调大 max_tokens 或在接口侧关闭思考模式。响应片段: {brief}"
            )));
        }
        return Err(AppError::Custom(format!(
            "AI 返回的 content 为空：可能是该模型不支持图片输入、响应结构不兼容或输出被截断。响应片段: {brief}"
        )));
    }

    // 解析出标题（兼容模型偶尔输出的列表/编号），只取第一条
    let titles = parse_titles(&content_text);
    let title = titles.into_iter().next().filter(|t| !t.is_empty()).ok_or_else(|| {
        AppError::Custom(format!(
            "AI 未返回有效标题，原始回复: {}",
            content_text.chars().take(300).collect::<String>()
        ))
    })?;
    Ok(title)
}

/// 从响应中提取模型正文，兼容多种返回结构。
/// 返回：(正文内容, 思考/推理内容)
fn extract_message_content(parsed: &Value) -> (String, Option<String>) {
    let as_text = |value: &Value| -> Option<String> {
        match value {
            // 忽略空串与 null（null 不能当标题）
            Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
            // 部分模型返回结构化 content（数组），拼接其中的文本片段
            Value::Array(items) => {
                let parts: Vec<String> = items
                    .iter()
                    .filter_map(|item| {
                        item.get("text")
                            .or_else(|| item.get("content"))
                            .and_then(|t| t.as_str())
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| s.to_string())
                    })
                    .collect();
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join("\n"))
                }
            }
            _ => None,
        }
    };

    let content = parsed
        .pointer("/choices/0/message/content")
        .and_then(as_text)
        // 部分兼容端点的备选字段
        .or_else(|| parsed.pointer("/choices/0/text").and_then(as_text))
        .or_else(|| parsed.pointer("/output_text").and_then(as_text))
        .or_else(|| parsed.pointer("/result").and_then(as_text))
        .unwrap_or_default();

    // 推理型模型（deepseek-reasoner、带思考开关的模型）可能把输出写在这里
    let reasoning = parsed
        .pointer("/choices/0/message/reasoning_content")
        .and_then(as_text);

    (content, reasoning)
}

/// 归一化思考强度：仅接受 DeepSeek 官方枚举（low / high / max），非法值回退到 low
fn normalize_reasoning_effort(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "high" => "high",
        "max" => "max",
        _ => "low",
    }
}

/// 从模型回复文本中提取标题列表（支持 JSON 数组 / 编号行 / 列表符号等）
fn parse_titles(raw: &str) -> Vec<String> {
    let mut cleaned = raw.trim().to_string();
    if cleaned.is_empty() {
        return Vec::new();
    }

    // 去除 markdown 代码块围栏
    if cleaned.starts_with("```") {
        cleaned = cleaned
            .trim_start_matches('`')
            .lines()
            .filter(|line| !line.trim_start().starts_with('`'))
            .collect::<Vec<_>>()
            .join("\n");
    }

    let mut titles: Vec<String> = Vec::new();

    // 尝试直接按 JSON 数组解析（优先）
    let try_parse_json = |s: &str| -> Option<Vec<String>> {
        let value: Value = serde_json::from_str(s).ok()?;
        let collect = |items: &Vec<Value>| -> Option<Vec<String>> {
            Some(
                items
                    .iter()
                    .filter_map(|item| {
                        item.as_str()
                            .or_else(|| item.get("title").and_then(|t| t.as_str()))
                            .map(|s| s.to_string())
                    })
                    .collect(),
            )
        };
        match value {
            Value::Array(items) => collect(&items),
            Value::Object(map) => {
                for key in ["titles", "标题", "data"] {
                    if let Some(Value::Array(items)) = map.get(key) {
                        return collect(items);
                    }
                }
                None
            }
            _ => None,
        }
    };

    if let Some(list) = try_parse_json(&cleaned) {
        for title in list {
            if let Some(t) = normalize_title(&title) {
                titles.push(t);
            }
        }
        return dedupe_titles(titles);
    }

    // 逐行解析
    for line in cleaned.lines() {
        if let Some(t) = normalize_title(line) {
            titles.push(t);
        }
    }

    dedupe_titles(titles)
}

/// 规范化单条标题行：去除编号/列表符号/引号等噪音
fn normalize_title(line: &str) -> Option<String> {
    let mut s = line.trim();
    if s.is_empty() {
        return None;
    }
    // 去除 markdown 行内残留
    if s.starts_with("```") || s.starts_with('`') {
        s = s.trim_matches('`').trim();
    }

    // 去掉成对引号包裹
    loop {
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();
        if len >= 2
            && ((chars[0] == '"' && chars[len - 1] == '"')
                || (chars[0] == '“' && chars[len - 1] == '”')
                || (chars[0] == '「' && chars[len - 1] == '」'))
        {
            s = &s[chars[0].len_utf8()..s.len() - chars[len - 1].len_utf8()];
            s = s.trim();
        } else {
            break;
        }
    }
    if s.is_empty() {
        return None;
    }

    // 去除列表符号、编号前缀，例如: "- 标题" / "• 标题" / "1. 标题" / "(1) 标题"
    let mut rest = s.trim_start();
    loop {
        let before = rest;
        let trimmed = rest.trim_start();
        if trimmed.is_empty() {
            return None;
        }
        let chars: Vec<char> = trimmed.chars().collect();
        let first = chars[0];
        match first {
            '-' | '*' => {
                rest = &trimmed[first.len_utf8()..];
            }
            '•' | '·' | '●' => {
                rest = &trimmed[first.len_utf8()..];
            }
            '(' | '（' => {
                let is_numbered = trimmed.char_indices().skip(1).find(|(_, c)| *c == ')' || *c == '）')
                    .map_or(false, |(pos, _)| {
                        trimmed[1..pos].trim().chars().all(|c| c.is_ascii_digit())
                    });
                if is_numbered {
                    if let Some((pos, _)) =
                        trimmed.char_indices().find(|(_, c)| *c == ')' || *c == '）')
                    {
                        rest = &trimmed[pos + 1..];
                    }
                } else {
                    break;
                }
            }
            c if c.is_ascii_digit() => {
                let digit_len = chars
                    .iter()
                    .take_while(|c| c.is_ascii_digit())
                    .map(|c| c.len_utf8())
                    .sum::<usize>();
                let after = &trimmed[digit_len..];
                let after_first = after.chars().next();
                match after_first {
                    Some('.') | Some('、') | Some(')') | Some('）') | Some(':') | Some('：') => {
                        rest = &after[after_first.unwrap().len_utf8()..];
                    }
                    _ => break,
                }
            }
            _ => break,
        }
        rest = rest.trim_start();
        if rest == before || rest.len() >= before.len() {
            break;
        }
    }

    let result = rest.trim();
    if result.is_empty() {
        return None;
    }

    // 过滤常见的引导性语句（如 “以下是为您生成的标题：xxx”），优先提取冒号后的实际内容
    const BOILERPLATE: [&str; 8] = [
        "以下", "下面是", "为您生成", "为你生成", "希望这", "好的", "收到", "请选择",
    ];
    if BOILERPLATE.iter().any(|p| result.starts_with(p)) {
        if let Some(idx) = result.find('：') {
            return normalize_title(&result[idx + 1..]);
        }
        return None;
    }

    // 去掉句末的中文标点残留（中文标题一般不带句号/感叹号结尾）
    let title: String = result
        .trim_end_matches(['。', '！', '，', '；'])
        .chars()
        .take(80)
        .collect();
    if title.is_empty() {
        return None;
    }
    Some(title)
}

/// 去重（保持顺序）
fn dedupe_titles(titles: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for title in titles {
        let key = title.to_lowercase();
        if seen.insert(key) {
            result.push(title);
        }
    }
    result
}
