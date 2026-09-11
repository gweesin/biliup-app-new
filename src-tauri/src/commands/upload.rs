use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::{
    AppData,
    commands::utils::{
        delete_local_video_file, query_video_cid, query_video_season, switch_season_inner,
    },
    error::AppError,
    models::{TemplateConfig, UploadTask, VideoInfo},
};
use tauri::{AppHandle, Manager};

use tracing::info;

/// submit 命令的返回体：B 站稿件数据 + 提交后一次性后处理（删除本地源文件 / 加入合集）结果。
/// 前端只需调用一次 submit，不再需要按顺序串行调用 delete_file / get_video_season / switch_season。
/// data 显式作为独立字段（而非 flatten），避免返回体缺失 data 时为 null 导致序列化失败。
#[derive(Debug, Clone, Serialize)]
pub struct SubmitOutcome {
    /// 稿件数据（新增/编辑接口返回体：aid / bvid / cid 等，可能为 null）
    pub data: Value,
    /// 本次已删除的本地视频文件路径列表
    #[serde(default)]
    pub deleted_file_paths: Vec<String>,
    /// 合集（season）处理结果
    pub season: SeasonOutcome,
}

/// 合集处理结果（投稿成功后由后端自动执行，前端只展示）
#[derive(Debug, Clone, Serialize)]
pub struct SeasonOutcome {
    /// 是否触发了合集处理
    pub attempted: bool,
    /// 是否成功（失败不影响投稿结果本身）
    pub ok: bool,
    /// 说明 / 失败原因
    pub message: String,
    /// 稿件原所属合集 id（0 表示未加入合集）
    pub old_season_id: u64,
    /// 目标合集 id
    pub new_season_id: u64,
}

impl Default for SeasonOutcome {
    fn default() -> Self {
        Self {
            attempted: false,
            ok: true,
            message: String::new(),
            old_season_id: 0,
            new_season_id: 0,
        }
    }
}

/// 创建上传任务
#[tauri::command]
pub async fn create_upload_task(
    app: AppHandle,
    uid: u64,
    template: String,
    video: VideoInfo,
) -> Result<bool, AppError> {
    let app_data = app.state::<AppData>();
    let user = app_data.get_client(uid).await?.user;
    let config_copy = Arc::clone(&app_data.config);
    let clients_copy = Arc::clone(&app_data.clients);
    let upload_service = &app_data.upload_service;

    let created = upload_service
        .create_task(&user, &template, &video, config_copy, clients_copy)
        .await
        .map_err(AppError::Internal)?;

    Ok(created)
}

/// 开始上传
#[tauri::command]
pub async fn start_upload(app: AppHandle, task_id: String) -> Result<bool, AppError> {
    let app_data = app.state::<AppData>();
    let upload_service = &app_data.upload_service;

    Ok(upload_service
        .start_upload(&task_id)
        .await
        .map_err(AppError::Internal)?)
}

/// 暂停上传
#[tauri::command]
pub async fn pause_upload(app: AppHandle, task_id: String) -> Result<bool, AppError> {
    let app_data = app.state::<AppData>();
    let upload_service = &app_data.upload_service;

    Ok(upload_service
        .pause_upload(&task_id)
        .await
        .map_err(AppError::Internal)?)
}

/// 取消上传
#[tauri::command]
pub async fn cancel_upload(app: AppHandle, task_id: String) -> Result<bool, AppError> {
    let app_data = app.state::<AppData>();
    let upload_service = &app_data.upload_service;

    Ok(upload_service
        .cancel_upload(&task_id)
        .await
        .map_err(AppError::Internal)?)
}

/// 获取上传队列
#[tauri::command]
pub async fn get_upload_queue(app: AppHandle) -> Result<Vec<UploadTask>, AppError> {
    let app_data = app.state::<AppData>();
    let upload_service = &app_data.upload_service;
    Ok(upload_service
        .get_upload_queue()
        .await
        .map_err(AppError::Internal)?)
}

/// 重新上传失败的任务
#[tauri::command]
pub async fn retry_upload(app: AppHandle, task_id: String) -> Result<bool, AppError> {
    let app_data = app.state::<AppData>();
    let upload_service = &app_data.upload_service;

    Ok(upload_service
        .retry_upload(&task_id)
        .await
        .map_err(AppError::Internal)?)
}

/// 提交（新增）或编辑稿件，并在内部一次性完成后处理：
/// ① 删除已上传稿件的本地源文件；② 把稿件加入/切换到目标合集。
/// 返回 `SubmitOutcome`：B 站稿件数据 + 后处理结果（deleted_file_paths / season）。
#[tauri::command]
pub async fn submit(app: AppHandle, uid: u64, form: TemplateConfig) -> Result<Value, AppError> {
    let app_data = app.state::<AppData>();
    // form 会被 into_bilibili_form 消费，后处理所需的字段提前克隆一份
    let post_form = form.clone();

    if form.aid.is_none() {
        // 将前端表单转换为B站API需要的格式
        let bilibili_form = form.into_bilibili_form();
        let studio = bilibili_form
            .try_into_studio()
            .map_err(AppError::Internal)?;

        #[cfg(debug_assertions)]
        {
            use tracing::debug;

            let json_content = serde_json::to_string_pretty(&studio).unwrap();
            debug!("转换后的B站提交表单: {uid}\n{}", json_content);
        }

        let proxy = app_data
            .config
            .lock()
            .await
            .config
            .get(&uid)
            .and_then(|c| c.proxy.clone());

        let bilibili = app_data.get_bilibili(uid).await?;
        match bilibili.submit_by_web(&studio, proxy.as_deref()).await {
            Ok(resp) => {
                info!("添加稿件成功：{resp}");
                let data = resp
                    .data
                    .ok_or_else(|| AppError::Biliup("返回值错误".to_string()))?;
                let outcome = finish_submit(&app_data, uid, &post_form, data).await?;
                Ok(serde_json::to_value(outcome)
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?)
            }
            Err(e) => Err(AppError::Internal(anyhow::anyhow!("{}", e))),
        }
    } else {
        let bilibili_form = form.into_bilibili_form();
        let studio = bilibili_form
            .try_into_studio()
            .map_err(AppError::Internal)?;
        let bilibili = app_data.get_bilibili(uid).await?;
        match bilibili.edit_by_web(&studio).await {
            Ok(resp) => {
                info!("编辑稿件成功：{resp}");
                let data = resp["data"].clone();
                let outcome = finish_submit(&app_data, uid, &post_form, data).await?;
                Ok(serde_json::to_value(outcome)
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("{e}")))?)
            }
            Err(e) => Err(AppError::Internal(anyhow::anyhow!("{}", e))),
        }
    }
}

/// 投稿成功后的一次性后处理：删除本地源文件 + 处理合集（全部在后端串行完成）。
/// 删除失败 / 合集失败都不影响投稿成功的结果，只记录日志或通过结果结构返回。
async fn finish_submit(
    app: &AppData,
    uid: u64,
    form: &TemplateConfig,
    data: Value,
) -> Result<SubmitOutcome, AppError> {
    // 1. 删除已上传稿件的原始本地文件（失败仅记录日志，不阻断投稿结果）
    let mut deleted_file_paths: Vec<String> = Vec::new();
    for video in &form.videos {
        let file_path = video.original_file_path.trim();
        if file_path.is_empty() {
            continue;
        }
        match delete_local_video_file(file_path) {
            Ok(true) => {
                info!("投稿成功后已删除本地源文件: {file_path}");
                deleted_file_paths.push(file_path.to_string());
            }
            Ok(false) => {
                info!("跳过删除本地源文件: {file_path}");
            }
            Err(e) => {
                info!("删除本地源文件失败(不影响投稿): {file_path}: {e}");
            }
        }
    }

    // 2. 合集处理：新增稿件未配置合集时跳过；编辑稿件或已配置合集时同步
    let season = handle_season_after_submit(app, uid, form, &data).await;

    Ok(SubmitOutcome {
        data,
        deleted_file_paths,
        season,
    })
}

/// 投稿/编辑成功后把稿件加入（或切换到）目标合集。
/// 规则与旧前端 syncSeasonAfterSubmit 保持一致：
/// - 新增稿件未配置 season_id → 跳过；
/// - 查询稿件当前合集 old_season_id，解析 cid，与目标合集不一致时才调用切换接口；
/// - 合集失败不使投稿失败，以结构返回结果供前端提示。
async fn handle_season_after_submit(
    app: &AppData,
    uid: u64,
    form: &TemplateConfig,
    data: &Value,
) -> SeasonOutcome {
    let mut outcome = SeasonOutcome {
        new_season_id: form.season_id.unwrap_or(0),
        ..SeasonOutcome::default()
    };

    // aid 优先取编辑表单，其次取投稿接口返回体
    let aid = form.aid.or_else(|| data.get("aid").and_then(|v| v.as_u64()));
    let Some(aid) = aid else {
        return outcome;
    };
    let configured_season_id = form.season_id.unwrap_or(0);
    let is_new = form.aid.is_none();
    if is_new && configured_season_id == 0 {
        return outcome;
    }

    outcome.attempted = true;

    // 查询稿件当前所属合集
    let old_season_id = match query_video_season(app, uid, aid).await {
        Ok(v) => v,
        Err(e) => {
            outcome.ok = false;
            outcome.message = format!("查询稿件当前合集失败: {e}");
            return outcome;
        }
    };
    outcome.old_season_id = old_season_id;

    if old_season_id == configured_season_id {
        // 已在目标合集，无需切换
        return outcome;
    }

    // 解析 cid：优先使用模板已保存的 cid，否则查接口（内部最多重试 3 次）
    let cid = form
        .videos
        .first()
        .map(|v| v.cid)
        .filter(|c| *c > 0)
        .unwrap_or(0);
    let cid = if cid > 0 {
        cid
    } else {
        match query_video_cid(app, uid, aid).await {
            Ok(c) if c > 0 => c,
            _ => {
                outcome.ok = false;
                outcome.message = "加入合集失败：未能获取稿件 cid".to_string();
                return outcome;
            }
        }
    };

    let add = old_season_id == 0;
    let section_id = form.section_id.unwrap_or(0);
    match switch_season_inner(
        app,
        uid,
        aid,
        cid,
        configured_season_id,
        section_id,
        &form.title,
        add,
    )
    .await
    {
        Ok(()) => {
            info!("稿件 {aid} 已加入合集 {configured_season_id} (add={add})");
            outcome.ok = true;
        }
        Err(e) => {
            outcome.ok = false;
            outcome.message = format!("设置合集失败: {e}");
        }
    }

    outcome
}
