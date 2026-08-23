# 项目长期记忆

## 项目概述
- biliup-app-new：基于 Tauri v2 + Vue 3 + Element Plus 的 B 站视频上传桌面客户端。
- 关键目录：`src/`（Vue 前端）、`src-tauri/`（Rust 后端）。
- 前端状态管理：Pinia（`src/stores/`，如 `user_config.ts`、`utils.ts`）。
- 全局配置：`ConfigRoot`（Rust 端 `src-tauri/src/models/user_config.rs`，前端 `src/stores/user_config.ts`），保存命令 `save_global_config`。

## 全局配置字段
- `max_curr`（最大并发任务数）、`auto_upload`、`auto_start`、`log_level`。
- `cover_match_path`（封面匹配路径，String，2026-08-23 新增）：存放封面图片的文件夹路径，用于按标题关键字自动匹配封面。

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
