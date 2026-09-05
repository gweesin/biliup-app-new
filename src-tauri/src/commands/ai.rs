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

/// 提交给 AI 的提示词模板
const AI_PROMPT: &str = "请处理这张 MOBA 游戏对局结算截图，按要求完成信息提取与标题生成：

提取核心信息
- 对局胜负结果：判断对局胜利 / 失败，以及双方阵营的最终比分
- 使用英雄：识别图片中绿底高亮行对应的英雄名称（仅提取英雄名，无需提取玩家用户名）
- 战绩 KDA：提取该英雄对应的「击杀数 / 死亡数 / 助攻数」

生成战报标题
基于提取到的英雄名和 KDA 战绩，生成 6-10 个有冲击力、适合游戏高光展示的标题，要求：
1. 每个标题必须包含英雄名称，英雄名称是三国人物名称，有的前面会包含 “梦”
2. 结合 KDA 突出战绩亮点（可从高击杀、零死亡、高助攻、全场 Carry、极致生存等不同角度创作）
3. 风格参考示例：
   14 杀张纮秀翻全场
   0 死张纮七进七出无人能敌
   张纮极致操作碾压全场
4. 标题简短有力，符合游戏社区的表达习惯，能够结合当下热点话题，吸引流量

请只输出生成的标题列表，每行一个标题，不要输出解释或前后缀文字。";

/// 截取视频（倒数第三秒）画面并请求 AI 生成标题，返回候选标题列表
#[tauri::command]
pub async fn generate_ai_titles(
    app: tauri::AppHandle,
    video_path: String,
) -> Result<Vec<String>, AppError> {
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

    // 4. 请求 OpenAI 兼容的视觉接口
    let titles = request_ai_titles(&ai, data_url).await?;
    Ok(titles)
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

/// 请求 OpenAI 兼容视觉接口，返回候选标题列表
async fn request_ai_titles(ai: &AiConfig, image_data_url: String) -> Result<Vec<String>, AppError> {
    let endpoint = build_chat_endpoint(&ai.base_url);
    if endpoint.is_empty() {
        return Err(AppError::Custom("AI 接口地址无效".to_string()));
    }

    let body = json!({
        "model": ai.model.trim(),
        "temperature": 0.8,
        "max_tokens": 1000,
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

    let parsed: Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Custom(format!("解析 AI 响应失败: {e}")))?;

    // 提取模型回复文本
    let content = parsed
        .pointer("/choices/0/message/content")
        .ok_or_else(|| AppError::Custom("AI 响应缺少 content 字段".to_string()))?;

    let content_text = match content {
        Value::String(s) => s.clone(),
        // 部分模型返回结构化 content（数组），拼接其中的文本片段
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    };

    let titles = parse_titles(&content_text);
    if titles.is_empty() {
        return Err(AppError::Custom(format!(
            "AI 未返回有效标题，原始回复: {}",
            content_text.chars().take(300).collect::<String>()
        )));
    }
    Ok(titles)
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

/// 去重（保持顺序），最多返回 12 条
fn dedupe_titles(titles: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for title in titles {
        let key = title.to_lowercase();
        if seen.insert(key) {
            result.push(title);
        }
        if result.len() >= 12 {
            break;
        }
    }
    result
}
