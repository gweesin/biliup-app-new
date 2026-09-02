import type { TemplateVideo } from './videoFileCleanup'

/**
 * 从视频的原始文件路径提取"默认标题"（文件名去扩展名）。
 * 导入视频时 title 默认取自文件名，若 title 与之相同则说明用户从未改过名。
 * 无路径信息时返回空串。
 */
export const getVideoOriginalTitle = (video?: TemplateVideo | null): string => {
    const filePath = video?.original_file_path || video?.path || ''
    if (!filePath) return ''
    const baseName = String(filePath).split(/[/\\]/).pop() || ''
    return baseName.replace(/\.[^/.]+$/, '')
}

/**
 * 判断视频标题是否已从默认文件名修改。
 * 无法获取原始文件名（无路径信息）时视为已处理，不阻塞。
 */
export const hasVideoTitleChanged = (video?: TemplateVideo | null): boolean => {
    const originalTitle = getVideoOriginalTitle(video)
    if (!originalTitle) return true
    return (video?.title || '').trim() !== originalTitle
}

/** 判断视频是否已设置封面 */
export const hasVideoCover = (video?: TemplateVideo | null): boolean => {
    return Boolean(String(video?.cover || '').trim())
}

/**
 * 分稿件提交就绪检查：
 * 上传完成 且 标题已修改（有原始路径可判断时）且 已设置封面。
 * 未就绪的视频即使上传进度 100% 也不会被提交。
 */
export const isVideoReadyForSeparateSubmit = (video?: TemplateVideo | null): boolean => {
    if (!video) return false
    if (!(video.complete && video.path === '')) return false
    if (!hasVideoTitleChanged(video)) return false
    if (!hasVideoCover(video)) return false
    return true
}

/**
 * 返回分稿件提交未就绪的原因（供 UI 提示），就绪时返回空串。
 */
export const getSeparateSubmitBlockReason = (video?: TemplateVideo | null): string => {
    if (!video || !(video.complete && video.path === '')) return ''

    const reasons: string[] = []
    if (!hasVideoTitleChanged(video)) {
        reasons.push('标题仍为默认文件名，请修改标题')
    }
    if (!hasVideoCover(video)) {
        reasons.push('请设置封面')
    }
    return reasons.join('，')
}
