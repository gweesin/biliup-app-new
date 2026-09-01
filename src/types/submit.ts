/** 稿件提交模式 */
export type SubmitModeText = '单稿件' | '多稿件'

/** 提交结果状态 */
export type SubmitStatus = 'success' | 'failed'

/** 提交结果状态文案 */
export type SubmitStatusText = '成功' | '失败'

/** 单条提交统计记录 */
export interface SubmitStatsRecord {
    time: string
    user: string
    mode: SubmitModeText
    templateName: string
    status: SubmitStatus
    statusText: SubmitStatusText
    bvid: string
    videoName: string
    error: string
}

/**
 * 记录一条提交统计所需的入参
 * time 与 statusText 由统计逻辑自动生成，bvid / videoName / error 可缺省
 */
export type SubmitStatsInput = Omit<
    SubmitStatsRecord,
    'time' | 'statusText' | 'bvid' | 'videoName' | 'error'
> & {
    bvid?: string
    videoName?: string
    error?: unknown
}

/** 提交统计汇总数据 */
export interface SubmitStats {
    startedAt: string
    totalCount: number
    successCount: number
    failCount: number
    records: SubmitStatsRecord[]
}

/** 多稿件（分P逐个提交）模式下单个模板的提交状态 */
export interface SeparateSubmitState {
    uid: number
    templateName: string
    attemptedVideoIds: Set<string>
    successCount: number
    failCount: number
    totalCount: number
    successBvids: string[]
    failedVideoNames: string[]
}
