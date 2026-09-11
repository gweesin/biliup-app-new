# 项目长期记忆

## 项目概述
- biliup-app-new：基于 Tauri v2 + Vue 3 + Element Plus 的 B 站视频上传桌面客户端。
- 关键目录：`src/`（Vue 前端）、`src-tauri/`（Rust 后端）。
- 前端状态管理：Pinia（`src/stores/`，如 `user_config.ts`、`utils.ts`）。
- 全局配置：`ConfigRoot`（Rust 端 `src-tauri/src/models/user_config.rs`，前端 `src/stores/user_config.ts`），保存命令 `save_global_config`。

## 全局配置字段
- `max_curr`（最大并发任务数）、`auto_upload`、`auto_start`、`log_level`。
- `cover_match_path`（封面匹配路径，String，2026-08-23 新增）：存放封面图片的文件夹路径，用于按标题关键字自动匹配封面。
- `ai`（AiConfig 对象，2026-09-05 新增，2026-09-07 拆分为两步双模型）：`{ enabled, ffmpeg_path, vision: AiEndpointConfig, writer: AiEndpointConfig }`；`AiEndpointConfig = { base_url, api_key, model, thinking, reasoning_effort }`，两端可指向不同服务商，ffmpeg 共用。默认 base_url=`https://api.openai.com/v1`；`thinking` 默认 true；`reasoning_effort` 默认 `"low"`（DeepSeek 官方取值 low/high/max）。旧扁平字段（ai.base_url 等）由自定义 Deserialize 一次性迁移到两端点。

## AI 标题生成功能（两步双模型，2026-09-07 重构）
- 架构：① `ai_analyze_video`（vision 端点：截帧 + 提取对局信息文本，temperature 1.0，返回 `{info, reasoning}`）→ ② `generate_ai_title`（writer 端点：纯文本请求，info 作为 system 上下文，temperature 1.2，返回 `{title, reasoning}`）。均在 `src-tauri/src/commands/ai.rs` 注册于 lib.rs。
- Kimi（moonshot 系）视觉模型要求 temperature 固定为 1，传其它值报参数错误（2026-09-11 已把 ai_analyze_video 的 temperature 从 0.2 改为 1.0）。
- 底层公共 `request_chat<F: FnMut(&str)>(endpoint, messages, temperature, on_reasoning)` 按 endpoint.thinking 自动走 SSE 流式/一次性；**async 命令内回调不可用 `&mut dyn FnMut`（future 非 Send），须用泛型闭包 F: FnMut**。
- 提示词在前端（`src/stores/utils.ts`）：`AI_VISION_PROMPT`（识别结算画面→每行一条的对局信息文本）、`AI_WRITE_PROMPT`（基于给定信息创作含英雄名的标题）；`AI_TITLE_PROMPT` 保留为 writer 别名。**改提示词只改前端常量，无需重编 Rust**。Rust 侧 prompt 为空会返回明确错误。
- 前端 store 方法：`utilsStore.analyzeVideo(videoPath, prompt?)`、`utilsStore.generateAiTitle(info, prompt?, onReasoning?)`。
- `GlobalConfig.vue`：AI 设置区分两个编号分组卡片（图像识别 / 标题生成），表单字段 `ai_vision_*` / `ai_writer_*`；`isAiConfigured` 需两端 base_url/api_key/model 均完整。
- `VideoList.vue`：sparkle 一键两步串联 → 回填 title；识别出的对局信息存 `aiInfos[videoId]` 展示于条目（`.ai-vision-info`，可 × 清除，随列表清理）。
- 注意：AI 截帧依赖本机 ffmpeg（两步共用 `ai.ffmpeg_path`）；DeepSeek 视觉模型 `deepseek-v4-flash-vision-exp` 支持 base64 图片，普通 deepseek-v4-* 不能识别图片；OpenAI 系不支持 thinking 参数需在设置中关闭。
- VideoList 传给 AI/删除等操作的本地路径用 `video.original_file_path`（`video.path` 上传后会清空）。

## 封面匹配功能（2026-08-23 实现）
- `GlobalConfig.vue`：新增"封面匹配路径"配置项，输入框 + "选择文件夹"按钮（`open({ directory: true })`，来自 `@tauri-apps/plugin-dialog`）。
- Rust 命令 `list_cover_images(dir_path, keywords)`（在 `src-tauri/src/commands/utils.rs`）：递归扫描目录（最多 5 层），返回文件名包含任一关键字的图片（jpg/jpeg/png/gif/webp/bmp，>8MB 跳过）的 `CoverImageItem { name, path, data_url }`，data_url 为 base64 预览。
- `MainView.vue`：
  - `COVER_MATCH_KEYWORDS`：三国英雄名称关键字列表（约 170 个）。
  - `refreshCoverMatch(title)`：watch 标题变化，300ms 防抖后调用 `list_cover_images`。
  - `setCoverFromMatch(filePath)`：点击匹配图片 → `utilsStore.uploadCover(uid, filePath)` → 设置模板 cover。
  - 匹配结果展示在封面 el-form-item 内、上传器下方，样式类 `.cover-match-list/.cover-match-grid/.cover-match-item`。

## 开发注意事项
- 修改 Rust 端配置字段需同步：`user_config.rs`（模型+Default+save_global_config）→ `commands/config.rs` → `src/stores/user_config.ts`（接口+updateGlobalConfig）→ `GlobalConfig.vue`。
- 新增 Tauri 命令需在 `src-tauri/src/lib.rs` 的 invoke_handler 中注册。
- **模板视频字段持久化（2026-09-06 踩坑）**：配置保存/读取会经 Rust「反序列化 → 序列化」往返，前端 `videos` 里的自定义字段（如 `original_file_path`、`cover`、`complete` 等）只要 `models/user_config.rs` 的 `VideoInfo` 未声明就会被丢弃（表现为「切换模板后字段消失」）。现已给 `VideoInfo` 增加显式字段 `original_file_path: String` 与 `#[serde(flatten)] extra: HashMap<String, Value>` 兜底所有扩展字段；新增需持久的前端视频字段请显式加到 `VideoInfo`（构造点仅有 `utils/compatible.rs` 旧配置迁移一处）。
- 提交兼容性：`submit` → `into_bilibili_form` 用 `json!(videos)` 序列化后反序列化为 biliup 的 `Studio`，未知字段会被忽略，因此 `extra` 不会真正提交给 B 站。
- dialog 权限（`dialog:default`、`dialog:allow-open`）已配置在 capabilities/default.json。
- 构建检查：前端 `npx vue-tsc --noEmit`；Rust `cargo check`。
