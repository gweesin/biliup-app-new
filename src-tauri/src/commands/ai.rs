use std::path::Path;
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde::Serialize;
use serde_json::{Value, json};
use tauri::Manager;
use tauri::ipc::Channel;
use tracing::{error, info};

use crate::error::AppError;
use crate::models::AiEndpointConfig;
use crate::utils::crypto::encode_base64;
use crate::utils::ffmpeg::{capture_frame_near_end, ffmpeg_queue_lock, resolve_ffmpeg};
use crate::{AppData, models::AiConfig};

// 提示词内容由前端传入：
// - 图像识别提示词见 src/stores/utils.ts 的 AI_VISION_PROMPT（发给 vision 模型）
// - 标题创作提示词见 AI_WRITE_PROMPT（发给 writer 模型）
// 修改提示词只需改动前端常量，无需重新编译 Rust。

/// 图像识别结果：从视频结算画面中提取出的对局信息文本
#[derive(Debug, Clone, Serialize)]
pub struct AiAnalyzeResult {
    pub info: String,
    pub reasoning: String,
}

/// AI 生成结果：标题 + 深度思考过程（reasoning_content，可能为空）
#[derive(Debug, Clone, Serialize)]
pub struct AiTitleResult {
    pub title: String,
    pub reasoning: String,
}

/// 步骤一：截取视频（倒数第三秒）画面，交给「图像识别」模型提取对局信息。
/// 识别提示词要求输出规范、可直接转交文本模型的纯文本，本命令不做标题创作。
/// 识别模型开启思考模式时，推理过程在后端聚合作为 reasoning 随结果返回，
/// 前端只用于日志/排查，不参与标题创作。
#[tauri::command]
pub async fn ai_analyze_video(
    app: tauri::AppHandle,
    video_path: String,
    prompt: String,
) -> Result<AiAnalyzeResult, AppError> {
    let video_path = video_path.trim().to_string();
    if video_path.is_empty() || !Path::new(&video_path).is_file() {
        return Err(AppError::Custom("视频文件不存在或路径无效".to_string()));
    }
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::Custom(
            "图像识别提示词为空：请升级前端版本后重试（提示词由前端传入，Rust 侧不内置）。"
                .to_string(),
        ));
    }

    let ai = app.state::<AppData>().config.lock().await.ai.clone();

    check_ai_enabled(&ai)?;
    check_endpoint(&ai.vision, "图像识别")?;

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
    //    并发的 AI 生成任务在此排队（网络请求阶段仍然并行，不受影响）
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
    info!(
        "视频画面截取成功 ({}), {} 字节",
        time_desc,
        frame_bytes.len()
    );

    // 3. 调用「图像识别」端点（视觉请求）。识别属信息抽取任务，temperature 取低值保证稳定
    let messages = build_vision_messages(&data_url, &prompt);
    let (content, reasoning) = request_chat(&ai.vision, messages, 0.2, |_| {}).await?;

    let info_text = content.trim().to_string();
    if info_text.is_empty() {
        return Err(AppError::Custom(
            "图像识别未返回有效内容：可能是该模型不支持图片输入、响应结构不兼容或输出被截断。\
             可在「全局设置 → AI 设置」中更换图像识别模型或关闭其思考模式后重试。"
                .to_string(),
        ));
    }
    info!("图像识别完成: 对局信息 {} 字符", info_text.chars().count());
    Ok(AiAnalyzeResult {
        info: info_text,
        reasoning: reasoning.trim().to_string(),
    })
}

/// 步骤二：把识别出的对局信息文本交给「标题生成」模型创作标题（纯文本请求）。
///
/// 开启「思考模式」时本命令走 SSE 流式请求，并把 reasoning_content 增量通过
/// `on_reasoning` 通道实时回传（payload: `{ "type": "reasoning", "text": "..." }`），
/// 前端据此在视频条目内展示「深度思考」过程。
#[tauri::command]
pub async fn generate_ai_title(
    app: tauri::AppHandle,
    info: String,
    prompt: String,
    on_reasoning: Channel<Value>,
) -> Result<AiTitleResult, AppError> {
    let info = info.trim().to_string();
    if info.is_empty() {
        return Err(AppError::Custom(
            "缺少图像识别出的对局信息：请先完成步骤一（截帧识别），再基于识别信息生成标题。"
                .to_string(),
        ));
    }
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::Custom(
            "标题创作提示词为空：请升级前端版本后重试（提示词由前端传入，Rust 侧不内置）。"
                .to_string(),
        ));
    }

    let ai = app.state::<AppData>().config.lock().await.ai.clone();

    check_ai_enabled(&ai)?;
    check_endpoint(&ai.writer, "标题生成")?;

    // 识别信息作为 system 上下文，创作要求作为 user 指令
    let messages = build_text_messages(&info, &prompt);

    // 标题创作属创意发散，temperature 取高值；开启思考模式时把推理增量实时回传 on_reasoning
    let (content, reasoning) = if ai.writer.thinking {
        request_chat(&ai.writer, messages, 1.2, move |delta| {
            let _ = on_reasoning.send(json!({ "type": "reasoning", "text": delta }));
        })
        .await?
    } else {
        request_chat(&ai.writer, messages, 1.2, |_| {}).await?
    };

    // 解析出标题（兼容模型偶尔输出的列表/编号），只取第一条
    let title = pick_first_title(&content)?;
    Ok(AiTitleResult {
        title,
        reasoning: reasoning.trim().to_string(),
    })
}

/// 校验 AI 功能总开关
fn check_ai_enabled(ai: &AiConfig) -> Result<(), AppError> {
    if !ai.enabled {
        return Err(AppError::Custom(
            "AI 标题生成尚未启用，请先在「全局设置 → AI 设置」中开启，\
             并分别配置「图像识别」与「标题生成」两个模型"
                .to_string(),
        ));
    }
    Ok(())
}

/// 校验单个端点（base_url / api_key / model）是否填写完整
fn check_endpoint(endpoint: &AiEndpointConfig, role: &str) -> Result<(), AppError> {
    if endpoint.base_url.trim().is_empty() {
        return Err(AppError::Custom(format!(
            "尚未配置{role}模型的接口地址，请先在「全局设置 → AI 设置」中填写其 Base URL"
        )));
    }
    if endpoint.api_key.trim().is_empty() {
        return Err(AppError::Custom(format!(
            "尚未配置{role}模型的接口密钥，请先在「全局设置 → AI 设置」中填写其 API Key"
        )));
    }
    if endpoint.model.trim().is_empty() {
        return Err(AppError::Custom(format!(
            "尚未配置{role}模型的名称，请先在「全局设置 → AI 设置」中填写其模型名"
        )));
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

/// 视觉请求的 messages：一段文字指令 + 一张截图
fn build_vision_messages(data_url: &str, prompt: &str) -> Value {
    json!([
        {
            "role": "user",
            "content": [
                { "type": "text", "text": prompt },
                { "type": "image_url", "image_url": { "url": data_url } }
            ]
        }
    ])
}

/// 纯文本请求的 messages：识别信息作为 system 上下文，创作要求作为 user 指令
fn build_text_messages(info: &str, prompt: &str) -> Value {
    json!([
        { "role": "system", "content": info },
        { "role": "user", "content": prompt }
    ])
}

/// 构建 OpenAI 兼容 chat/completions 的请求体
fn build_chat_payload(
    endpoint: &AiEndpointConfig,
    messages: &Value,
    stream: bool,
    temperature: f64,
) -> Value {
    let mut body = json!({
        "model": endpoint.model.trim(),
        "temperature": temperature,
        "stream": stream,
        "messages": messages,
    });

    // 思考模式（DeepSeek 等接口）：显式下发 thinking 参数开启。
    // 这些参数非 OpenAI 标准，不支持的接口请在「全局设置 → AI 设置」中关闭思考模式
    if endpoint.thinking {
        let effort = normalize_reasoning_effort(&endpoint.reasoning_effort);
        body["thinking"] = json!({ "type": "enabled" });
        body["reasoning_effort"] = json!(effort);
        // 不限制 max_tokens：交由接口/模型侧自行决定输出长度，
        // 避免思考模式下因硬性截断只返回 reasoning 而没有正文
        info!(
            "AI 请求已开启思考模式 (thinking=enabled, reasoning_effort={effort}, 不限制 max_tokens, stream={stream})"
        );
    }
    body
}

/// 创建 HTTP 客户端并发送请求（120s 超时，透传 api_key 做 Bearer 鉴权）
async fn send_ai_request(
    endpoint: &AiEndpointConfig,
    endpoint_url: &str,
    body: &Value,
) -> Result<reqwest::Response, AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| AppError::Custom(format!("创建 HTTP 客户端失败: {e}")))?;

    let mut request = client.post(endpoint_url).json(body);
    let api_key = endpoint.api_key.trim();
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }

    info!(
        "请求 AI 接口: {endpoint_url}, 模型: {}, 请求数据: {}",
        endpoint.model.trim(),
        summarize_request_body(body)
    );
    match request.send().await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            error!("请求 AI 接口失败: {e}");
            Err(AppError::Custom(format!("请求 AI 接口失败: {e}")))
        }
    }
}

/// 统一对话入口：按端点的 thinking 开关自动选择「SSE 流式」或「一次性」请求，
/// 返回 (正文 content, 思考内容 reasoning)。
/// `on_reasoning` 为同步回调：流式请求时每个 reasoning 增量都会调用一次，
/// 由调用方决定是否转发给前端（标题生成阶段会经 Channel 实时回传）。
/// 使用泛型回调（而非 dyn 引用），保证命令 future 可跨线程（Send）。
async fn request_chat<F: FnMut(&str)>(
    endpoint: &AiEndpointConfig,
    messages: Value,
    temperature: f64,
    on_reasoning: F,
) -> Result<(String, String), AppError> {
    let endpoint_url = build_chat_endpoint(&endpoint.base_url);
    if endpoint_url.is_empty() {
        return Err(AppError::Custom("AI 接口地址无效".to_string()));
    }

    let body = build_chat_payload(endpoint, &messages, endpoint.thinking, temperature);
    let response = send_ai_request(endpoint, &endpoint_url, &body).await?;

    if endpoint.thinking {
        let mut on_reasoning = on_reasoning;
        request_chat_streaming_body(response, &mut on_reasoning).await
    } else {
        request_chat_once_body(response).await
    }
}

/// 一次性（非流式）响应处理：等待完整 JSON 响应后返回 (正文, 完整思考内容)
async fn request_chat_once_body(
    response: reqwest::Response,
) -> Result<(String, String), AppError> {
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
    check_content_not_empty(&content_text, reasoning_text.as_deref(), &text)?;
    Ok((content_text, reasoning_text.unwrap_or_default()))
}

/// 在 SSE 字节流中查找事件结束符 \n\n 的位置（返回第二个 \n 的下标）
fn find_sse_delimiter(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n").map(|i| i + 1)
}

/// 从单个 SSE 事件里提取 reasoning / content 增量
/// （优先流式 delta 字段，兼容整帧 message 字段兜底）
fn extract_stream_deltas(parsed: &Value) -> (String, String) {
    let pick = |paths: &[&str]| -> String {
        paths
            .iter()
            .find_map(|path| parsed.pointer(path).and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .unwrap_or_default()
    };
    let reasoning = pick(&[
        "/choices/0/delta/reasoning_content",
        "/choices/0/message/reasoning_content",
    ]);
    let content = pick(&["/choices/0/delta/content", "/choices/0/message/content"]);
    (reasoning, content)
}

/// 处理一个完整的 SSE 事件帧：聚合 reasoning/content，并把推理增量实时回调给调用方。
/// 返回是否收到 [DONE]。
fn handle_sse_event(
    raw: &[u8],
    on_reasoning: &mut dyn FnMut(&str),
    reasoning: &mut String,
    content: &mut String,
) -> bool {
    let text = String::from_utf8_lossy(raw);
    let mut data_lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim());
        }
    }
    if data_lines.is_empty() {
        return false;
    }
    let joined = data_lines.join("\n");
    if joined.trim() == "[DONE]" {
        return true;
    }
    let Ok(parsed) = serde_json::from_str::<Value>(&joined) else {
        return false;
    };
    let (reasoning_delta, content_delta) = extract_stream_deltas(&parsed);
    if !reasoning_delta.is_empty() {
        reasoning.push_str(&reasoning_delta);
        on_reasoning(&reasoning_delta);
    }
    if !content_delta.is_empty() {
        content.push_str(&content_delta);
    }
    false
}

/// SSE 流式响应处理：把 reasoning_content 增量实时回调给调用方，
/// 结束后返回 (正文, 完整思考内容)
async fn request_chat_streaming_body<F: FnMut(&str)>(
    response: reqwest::Response,
    on_reasoning: &mut F,
) -> Result<(String, String), AppError> {
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        error!("AI 接口返回错误 (HTTP {status}), 完整响应: {text}");
        let brief: String = text.chars().take(500).collect();
        return Err(AppError::Custom(format!(
            "AI 接口返回错误 (HTTP {status}): {brief}"
        )));
    }
    info!("AI 流式连接成功 (HTTP {status})");

    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    let mut raw_body = String::new();
    let mut reasoning = String::new();
    let mut content = String::new();
    let mut done = false;

    while !done {
        match stream.next().await {
            Some(Ok(bytes)) => {
                buffer.extend_from_slice(&bytes);
                // 完整保留原始响应，便于整段 JSON 兜底解析
                raw_body.push_str(&String::from_utf8_lossy(&bytes));
                // 一个 chunk 可能携带多个事件，逐个消费到缓冲区不足一个完整事件为止
                loop {
                    match find_sse_delimiter(&buffer) {
                        Some(end) => {
                            let event: Vec<u8> = buffer.drain(..=end).collect();
                            if handle_sse_event(&event, on_reasoning, &mut reasoning, &mut content)
                            {
                                done = true;
                            }
                        }
                        None => break,
                    }
                    if done {
                        break;
                    }
                }
            }
            Some(Err(e)) => {
                error!("读取 AI 流失败: {e}");
                return Err(AppError::Custom(format!("读取 AI 流失败: {e}")));
            }
            // EOF：处理缓冲区里可能残留的最后一帧（个别流不以 \n\n 收尾）
            None => {
                if !buffer.is_empty() {
                    raw_body.push_str(&String::from_utf8_lossy(&buffer));
                    let tail = std::mem::take(&mut buffer);
                    handle_sse_event(&tail, on_reasoning, &mut reasoning, &mut content);
                }
                break;
            }
        }
    }

    // 个别网关会忽略 stream=true 直接返回整段 JSON（非 SSE）：作整段兜底解析
    if reasoning.trim().is_empty() && content.trim().is_empty() && !raw_body.trim().is_empty() {
        if let Ok(parsed) = serde_json::from_str::<Value>(raw_body.trim()) {
            let (c, r) = extract_message_content(&parsed);
            if !c.trim().is_empty() {
                content = c;
                reasoning = r.unwrap_or_default();
                info!("AI 网关未按 SSE 流式返回，已按整段 JSON 兜底解析成功");
            }
        }
    }

    let reasoning_out = reasoning.trim().to_string();
    let content_out = content.trim().to_string();
    if content_out.is_empty() {
        if reasoning_out.is_empty() {
            return Err(AppError::Custom(
                "AI 流式返回异常：未收到任何内容。请检查模型是否兼容 OpenAI chat/completions \
                 格式（图像识别步骤还要求支持图片输入）与流式输出，或在「全局设置 → AI 设置」\
                 中关闭对应模型的思考模式后重试。"
                    .to_string(),
            ));
        }
        let brief: String = reasoning_out.chars().take(300).collect();
        return Err(AppError::Custom(format!(
            "AI 只返回了思考内容（reasoning）而没有正文，通常是接口侧的输出长度上限被耗尽。\
             可在接口侧关闭思考模式或放宽输出限制后重试。思考片段: {brief}"
        )));
    }

    info!(
        "AI 流式结束：思考内容 {} 字符, 正文 {} 字符",
        reasoning_out.chars().count(),
        content_out.chars().count()
    );
    Ok((content_out, reasoning_out))
}

/// 校验正文为空的情况（区分「只思考没正文」与「完全没内容」）
fn check_content_not_empty(
    content_text: &str,
    reasoning_text: Option<&str>,
    raw_text: &str,
) -> Result<(), AppError> {
    if !content_text.trim().is_empty() {
        return Ok(());
    }
    error!("AI 返回的 content 为空, 完整响应: {raw_text}");
    let brief: String = raw_text.chars().take(800).collect();
    if reasoning_text.map(|r| !r.trim().is_empty()).unwrap_or(false) {
        return Err(AppError::Custom(format!(
            "AI 只返回了思考内容（reasoning）而没有正文，通常是接口侧的输出长度上限被耗尽。\
             可在「全局设置 → AI 设置」中关闭对应模型的思考模式或放宽输出限制后重试。响应片段: {brief}"
        )));
    }
    Err(AppError::Custom(format!(
        "AI 返回的 content 为空：可能是该模型不支持图片输入（图像识别步骤）、响应结构不兼容或输出被截断。响应片段: {brief}"
    )))
}

/// 生成用于日志输出的请求数据：将超长的 base64 图片 data_url 压缩为摘要，
/// 其余字段（model / messages / prompt / thinking 等）原样完整展示，便于命令行排查。
fn summarize_request_body(body: &Value) -> String {
    let mut body = body.clone();
    let mask_image = |node: &mut Value| {
        if let Some(url) = node.pointer_mut("/image_url/url").and_then(|v| v.as_str()) {
            if url.len() > 300 {
                let (prefix, _) = url.split_at(url.len().min(60));
                let total = url.len();
                *node.pointer_mut("/image_url/url").unwrap() = json!(format!(
                    "{prefix}… [base64 图片, 共 {total} 字符, 日志中省略]"
                ));
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
            // 忽略空串与 null
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

    // 推理型模型可能把输出写在这里
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

/// 从模型回复正文中提取第一条标题（兼容列表/编号等噪音）
fn pick_first_title(content: &str) -> Result<String, AppError> {
    let titles = parse_titles(content);
    titles
        .into_iter()
        .next()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| {
            AppError::Custom(format!(
                "标题生成模型未返回有效标题，原始回复: {}",
                content.chars().take(300).collect::<String>()
            ))
        })
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
                let is_numbered = trimmed
                    .char_indices()
                    .skip(1)
                    .find(|(_, c)| *c == ')' || *c == '）')
                    .map_or(false, |(pos, _)| {
                        trimmed[1..pos].trim().chars().all(|c| c.is_ascii_digit())
                    });
                if is_numbered {
                    if let Some((pos, _)) = trimmed
                        .char_indices()
                        .find(|(_, c)| *c == ')' || *c == '）')
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

    // 过滤常见的引导性语句，优先提取冒号后的实际内容
    const BOILERPLATE: [&str; 8] = [
        "以下", "下面是", "为您生成", "为你生成", "希望这", "好的", "收到", "请选择",
    ];
    if BOILERPLATE.iter().any(|p| result.starts_with(p)) {
        if let Some(idx) = result.find('：') {
            return normalize_title(&result[idx + 1..]);
        }
        return None;
    }

    // 去掉句末的中文标点残留
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
