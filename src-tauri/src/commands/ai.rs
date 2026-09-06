use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tauri::Manager;
use tracing::{error, info};

use crate::error::AppError;
use crate::utils::crypto::encode_base64;
use crate::utils::ffmpeg::{capture_frame_near_end, ffmpeg_queue_lock, resolve_ffmpeg};
use crate::{AppData, models::AiConfig};

// 提示词内容由前端传入（见 src/stores/utils.ts 的 AI_TITLE_PROMPT），
// 修改提示词无需重新编译 Rust；前端在调用 generate_ai_title 时作为 prompt 参数下发。

/// 截取视频（倒数第三秒）画面并请求 AI 生成一个标题
#[tauri::command]
pub async fn generate_ai_title(
    app: tauri::AppHandle,
    video_path: String,
    prompt: String,
) -> Result<String, AppError> {
    let video_path = video_path.trim().to_string();
    if video_path.is_empty() || !Path::new(&video_path).is_file() {
        return Err(AppError::Custom("视频文件不存在或路径无效".to_string()));
    }
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::Custom(
            "AI 提示词为空：请升级前端版本后重试（提示词由前端传入，Rust 侧已不内置）。".to_string(),
        ));
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

    // 2. 截取倒数第三秒画面：ffmpeg 调用进入串行队列，同一时刻只跑一个进程，
    // 并发的 AI 生成任务在此排队（网络请求阶段仍然并行，不受影响）
    let queue_wait_start = Instant::now();
    let (time_desc, frame_bytes) = {
        let _queue_guard = ffmpeg_queue_lock().await;
        let waited = queue_wait_start.elapsed();
        if waited.as_millis() >= 200 {
            info!(
                "ffmpeg 队列等待 {:.3}s 后开始执行, 路径: {video_path}",
                waited.as_secs_f64()
            );
        }
        capture_frame_near_end(&ffmpeg, &video_path, 3.0).await?
    };
    if frame_bytes.is_empty() {
        return Err(AppError::Custom("截取视频画面失败，输出为空".to_string()));
    }
    let data_url = format!("data:image/jpeg;base64,{}", encode_base64(&frame_bytes));
    info!("视频画面截取成功 ({}), {} 字节", time_desc, frame_bytes.len());

    // 3. 请求 OpenAI 兼容的视觉接口，取 AI 生成的单个标题
    let title = request_ai_title(&ai, data_url, prompt).await?;
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
async fn request_ai_title(
    ai: &AiConfig,
    image_data_url: String,
    prompt: String,
) -> Result<String, AppError> {
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
                    { "type": "text", "text": prompt },
                    {
                        "type": "image_url",
                        "image_url": { "url": image_data_url }
                    }
                ]
            }
        ]
    });

    // 思考模式（DeepSeek 等接口）：显式下发 thinking 参数开启。
    // 提示词要求模型先在脑中构思 5 个方向再做取舍，需要较充足的思考预算。
    // 注意：这些参数非 OpenAI 标准，不支持的接口请在「全局设置 → AI 设置」中关闭思考模式
    if ai.thinking {
        let effort = normalize_reasoning_effort(&ai.reasoning_effort);
        body["thinking"] = json!({ "type": "enabled" });
        body["reasoning_effort"] = json!(effort);
        // 不限制 max_tokens：不显式设置输出长度上限，交由接口/模型侧自行决定，
        // 避免思考模式下因硬性截断只返回 reasoning 而没有正文
        info!("AI 请求已开启思考模式 (thinking=enabled, reasoning_effort={effort}, 不限制 max_tokens)");
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

    info!(
        "请求 AI 接口: {endpoint}, 模型: {}, 请求数据: {}",
        ai.model.trim(),
        summarize_request_body(&body)
    );
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
        error!("AI 接口返回错误 (HTTP {status}), 完整响应: {text}");
        let brief: String = text.chars().take(500).collect();
        return Err(AppError::Custom(format!(
            "AI 接口返回错误 (HTTP {status}): {brief}"
        )));
    }

    // 记录完整响应，便于命令行排查（不截断）
    info!("AI 响应 (HTTP {status}): {text}");

    let parsed: Value = serde_json::from_str(&text).map_err(|e| {
        error!("解析 AI 响应失败: {e}; 原始响应: {text}");
        AppError::Custom(format!(
            "解析 AI 响应失败: {e}; 原始响应: {}",
            text.chars().take(500).collect::<String>()
        ))
    })?;

    let (content_text, reasoning_text) = extract_message_content(&parsed);
    if content_text.trim().is_empty() {
        error!("AI 返回的 content 为空, 完整响应: {text}");
        let brief: String = text.chars().take(800).collect();
        if reasoning_text.is_some() {
            return Err(AppError::Custom(format!(
                "AI 只返回了思考内容（reasoning）而没有正文，通常是接口侧的输出长度上限被耗尽。\
                 可在接口侧关闭思考模式或放宽输出限制后重试。响应片段: {brief}"
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

/// 生成用于日志输出的完整请求数据：将超长的 base64 图片 data_url 压缩为摘要，
/// 其余字段（model / messages / prompt / thinking 等）原样完整展示，便于命令行排查。
fn summarize_request_body(body: &Value) -> String {
    let mut body = body.clone();
    let mask_image = |node: &mut Value| {
        if let Some(url) = node
            .pointer_mut("/image_url/url")
            .and_then(|v| v.as_str())
        {
            if url.len() > 300 {
                let (prefix, _) = url.split_at(url.len().min(60));
                let total = url.len();
                *node.pointer_mut("/image_url/url").unwrap() =
                    json!(format!("{prefix}… [base64 图片, 共 {total} 字符, 日志中省略]"));
            }
        }
    };

    if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for msg in messages.iter_mut() {
            if let Some(parts) = msg.get_mut("content").and_then(|c| c.as_array_mut()) {
                for part in parts.iter_mut() {
                    mask_image(part);
                }
            }
            if let Some(part) = msg.get_mut("content").and_then(|c| c.as_object_mut()) {
                if let Some(img) = part.get_mut("image_url") {
                    mask_image(img);
                }
            }
        }
    }

    serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string())
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
