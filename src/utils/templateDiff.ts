import type { TemplateConfig } from '../stores/user_config'

/** 模板配置中参与「是否有未保存改动」比对的关键字段 */
const TEMPLATE_FIELDS_TO_COMPARE = [
    'title',
    'cover',
    'copyright',
    'source',
    'tid',
    'tid_v2',
    'desc',
    'desc_v2',
    'dynamic',
    'tag',
    'dtime',
    'open_subtitle',
    'interactive',
    'mission_id',
    'topic_id',
    'topic_name',
    'season_id',
    'section_id',
    'dolby',
    'lossless_music',
    'no_reprint',
    'open_elec',
    'no_disturbance',
    'up_selection_reply',
    'up_close_reply',
    'up_close_danmu',
    'is_only_self',
    'watermark',
    'is_360',
    'staff',
    'state',
    'state_desc'
]

/** 视频条目中参与比对的字段 */
const VIDEO_FIELDS_TO_COMPARE = ['title', 'filename', 'desc', 'path', 'cid']

const isPlainObject = (value: unknown): value is Record<string, any> => {
    return Object.prototype.toString.call(value) === '[object Object]'
}

/** 递归排序对象键，消除键顺序差异对比较结果的影响 */
const normalizeForCompare = (value: any): any => {
    if (Array.isArray(value)) {
        return value.map(item => normalizeForCompare(item))
    }

    if (isPlainObject(value)) {
        const sorted: Record<string, any> = {}
        for (const key of Object.keys(value).sort()) {
            sorted[key] = normalizeForCompare(value[key])
        }
        return sorted
    }

    return value
}

/** 稳定序列化：键顺序无关，可安全用于两值比较 */
const stableStringify = (value: any): string => {
    return JSON.stringify(normalizeForCompare(value))
}

/** 判断当前模板相对基准快照是否存在未保存的改动 */
export const hasUnsavedChanges = (
    baseTemplateData: TemplateConfig,
    currentTemplateData: TemplateConfig
): boolean => {
    // 比较关键字段
    for (const field of TEMPLATE_FIELDS_TO_COMPARE) {
        const currentValue = (currentTemplateData as any)[field]
        const baseValue = (baseTemplateData as any)[field]

        // 处理 undefined/null/空字符串 的情况
        if (
            (currentValue === undefined || currentValue === null || currentValue === '') &&
            (baseValue === undefined || baseValue === null || baseValue === '')
        ) {
            continue
        }

        if (stableStringify(currentValue) !== stableStringify(baseValue)) {
            return true
        }
    }

    // 特别比较 videos 数组
    const currentVideos = currentTemplateData.videos || []
    const baseVideos = baseTemplateData.videos || []

    if (currentVideos.length !== baseVideos.length) {
        return true
    }

    // 比较视频的关键字段
    for (let i = 0; i < currentVideos.length; i++) {
        const currentVideo = currentVideos[i]
        const baseVideo = baseVideos[i]

        for (const field of VIDEO_FIELDS_TO_COMPARE) {
            if (
                stableStringify((currentVideo as any)[field]) !==
                stableStringify((baseVideo as any)[field])
            ) {
                return true
            }
        }
    }

    return false
}

interface TemplateConfigSource {
    config?: Record<number | string, { templates?: Record<string, TemplateConfig> }>
}

/** 判断指定模板是否存在未保存的改动（从配置根节点中取出当前值与基准值比较） */
export const hasTemplateUnsavedChanges = (
    configRoot: TemplateConfigSource | null | undefined,
    configBase: TemplateConfigSource | null | undefined,
    uid: number,
    templateName: string
): boolean => {
    if (!configRoot?.config || !configBase?.config) {
        return false
    }

    const currentUserConfig = configRoot.config[uid]
    const baseUserConfig = configBase.config[uid]

    if (
        !currentUserConfig?.templates?.[templateName] ||
        !baseUserConfig?.templates?.[templateName]
    ) {
        return false
    }

    return hasUnsavedChanges(
        baseUserConfig.templates[templateName],
        currentUserConfig.templates[templateName]
    )
}
