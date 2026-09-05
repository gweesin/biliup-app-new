# 项目长期记忆

## 项目概述
- biliup-app-new：基于 Tauri v2 + Vue 3 + Element Plus 的 B 站视频上传桌面客户端。
- 关键目录：`src/`（Vue 前端）、`src-tauri/`（Rust 后端）。
- 前端状态管理：Pinia（`src/stores/`，如 `user_config.ts`、`utils.ts`）。
- 全局配置：`ConfigRoot`（Rust 端 `src-tauri/src/models/user_config.rs`，前端 `src/stores/user_config.ts`），保存命令 `save_global_config`。

## 全局配置字段
- `max_curr`（最大并发任务数）、`auto_upload`、`auto_start`、`log_level`。
- `cover_match_path`（封面匹配路径，String，2026-08-23 新增）：存放封面图片的文件夹路径，用于按标题关键字自动匹配封面。
- `ai`（AiConfig 对象，2026-09-05 新增）：`{ enabled, base_url, api_key, model, ffmpeg_path }`，OpenAI 兼容接口配置，默认 base_url=`https://api.openai.com/v1`。

## AI 标题生成功能（2026-09-05 实现）
- `GlobalConfig.vue`：新增"AI 设置"分区（开启开关、Base URL、API Key 密码框、模型名、ffmpeg 路径+选择文件按钮）。
- Rust 命令 `generate_ai_titles(video_path)`（`src-tauri/src/commands/ai.rs`）：校验配置 → `resolve_ffmpeg`（配置路径→PATH→Windows 常见目录）→ ffprobe/ffmpeg 探测时长 → `ffmpeg -ss dur-3 -frames:v 1 -c:v mjpeg` 截帧 → 请求 `{base}/chat/completions` 视觉接口（Bearer，120s 超时，prompt 常量 AI_PROMPT 要求提取英雄+KDA 生成 6-10 个标题）→ 清洗返回候选标题（JSON 数组/编号行，去重最多 12 条）。
- `VideoList.vue`：`.video-title` 末尾 sparkle svg（类 `.ai-icon`，紫色 hover 发光）→ 点击弹候选列表对话框（`.ai-candidate-*`）→ 点击候选通过 `emit('update:videos')` 回填 title。
- 注意：AI 截帧依赖本机 ffmpeg；未配置/未安装时命令返回中文错误提示，需引导用户前往全局设置。
- 使用要点（2026-09-05 查证）：OpenAI 兼容 /chat/completions 的 `model` 均为必填（含 DeepSeek 官方 API），不可留空；DeepSeek 视觉模型名为 `deepseek-v4-flash-vision-exp`（支持 base64 图片，普通 deepseek-v4-* 无法识别图片）；设置页模型名 placeholder 已含该示例。
- VideoList 中传给 AI 截帧/删除源文件等操作的本地路径应使用 `video.original_file_path`（`video.path` 在上传完成后会被清空，两者语义不同），2026-09-05 已修正 `handleAiGenerateTitle` 两处取值。

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
- dialog 权限（`dialog:default`、`dialog:allow-open`）已配置在 capabilities/default.json。
- 构建检查：前端 `npx vue-tsc --noEmit`；Rust `cargo check`。
