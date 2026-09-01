import { invoke } from '@tauri-apps/api/core'

/** 模板配置中的视频条目 */
export interface TemplateVideo {
    id?: string
    filename?: string
    path?: string
    /** 原始本地文件路径，稿件发布成功后用于删除源文件 */
    original_file_path?: string
    complete?: boolean
    finished_at?: number
    [key: string]: any
}

/** 上传队列任务中与文件清理相关的字段 */
export interface UploadTaskLike {
    template: string
    status: string
    user?: { uid?: number | string } | null
    video?: TemplateVideo | null
    finished_at?: number
}

/** 用户配置根节点中与文件清理相关的字段 */
export interface ConfigRootLike {
    config?: Record<number | string, { templates?: Record<string, { videos?: TemplateVideo[] }> }>
}

const COMPLETED_STATUS = 'Completed'

/**
 * 删除已发布（稿件提交成功）视频的原始本地文件
 * 删除失败仅记录日志，不向上抛出，避免影响投稿主流程
 * @returns 是否实际执行了删除
 */
export const deleteOriginalVideoFile = async (video?: TemplateVideo | null): Promise<boolean> => {
    const filePath = video?.original_file_path
    if (!filePath) {
        return false
    }

    try {
        await invoke('delete_file', { filePath })
        console.log('已删除原始视频文件:', filePath)
        video.original_file_path = ''
        return true
    } catch (error) {
        console.error('删除原始视频文件失败:', filePath, error)
        return false
    }
}

/**
 * 批量删除已发布视频的原始本地文件
 * @returns 成功删除的文件数量
 */
export const deleteOriginalVideoFiles = async (
    videos?: TemplateVideo[] | null
): Promise<number> => {
    if (!videos?.length) {
        return 0
    }

    let deletedCount = 0
    for (const video of videos) {
        if (await deleteOriginalVideoFile(video)) {
            deletedCount++
        }
    }
    return deletedCount
}

/**
 * 将已完成任务的文件信息同步到模板配置
 * @returns 同步的视频数量
 */
export const syncCompletedTasksToConfig = (
    configRoot?: ConfigRootLike | null,
    tasks?: UploadTaskLike[] | null
): number => {
    const config = configRoot?.config
    if (!config || !tasks?.length) {
        return 0
    }

    let syncedCount = 0
    for (const task of tasks) {
        if (task.status !== COMPLETED_STATUS) {
            continue
        }

        const templateName = task.template
        const uid = task.user?.uid
        if (templateName == null || uid == null) {
            continue
        }

        const videos = config[uid]?.templates?.[templateName]?.videos || []
        const taskVideo = task.video
        const video = videos.find(v => v.id === taskVideo?.id)
        if (!video || !taskVideo || video.filename === taskVideo.filename) {
            continue
        }

        // 上传完成后 task.video.path 已被清空，先暂存原始本地路径，待稿件发布成功后删除
        if (!video.original_file_path && video.path) {
            video.original_file_path = video.path
        }
        video.filename = taskVideo.filename
        video.path = taskVideo.path
        video.complete = true
        video.finished_at = task.finished_at
        syncedCount++
    }
    return syncedCount
}
