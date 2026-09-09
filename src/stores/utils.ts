import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { Channel, invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'
import type { MentionUserGroup } from '../types/mention'

/** Rust 端 ai_analyze_video 返回的结构化结果（步骤一：图像识别） */
export interface AiAnalyzeResult {
    /** 从结算画面中提取出的对局信息文本 */
    info: string
    /** 图像识别模型的思考过程（可能为空，仅用于排查） */
    reasoning: string
}

/** Rust 端 generate_ai_title 返回的结构化结果（步骤二：标题创作） */
export interface AiTitleResult {
    title: string
    reasoning: string
}

/** Rust 端通过 on_reasoning 通道实时回传的事件载荷 */
export interface AiReasoningEvent {
    type: 'reasoning'
    text: string
}

/**
 * 步骤一（图像识别模型）的提示词，存放在前端以便直接修改（改动无需重新编译 Rust）。
 * 发给「图像识别」端点（要求支持图片输入），只负责从结算画面中提取对局信息文本。
 */
export const AI_VISION_PROMPT = `请识别这张 MOBA 游戏梦三国2 的对局结算截图，提取对局信息。

需要提取的内容：
- 对局胜负结果与双方阵营的最终比分
- 绿底高亮行对应的英雄名称（只取英雄名；英雄名多为三国人物名，不要带玩家名）
- 该英雄的 KDA（击杀 / 死亡 / 助攻）
- 其他你认为对后续创作标题有价值且确信无误的对局事实

输出要求：
- 只输出提取出的对局信息（忽略对局时长、模式、积分）
- 信息不确定时不要编造，直接省略或标注「不确定」
- 你的输出将作为上下文原样交给另一个 AI 创作标题，因此请直接给出信息正文`
// - 不要输出 JSON 包裹、不要创作标题、不要任何寒暄或解释性前后缀文字`

/**
 * 步骤二（标题生成模型）的提示词，存放在前端以便直接修改（改动无需重新编译 Rust）。
 * 发给「标题生成」端点（纯文本能力即可），根据给定的对局信息创作标题。
 */
export const AI_WRITE_PROMPT = `你是一名资深的《梦三国2》游戏剪辑标题编辑。系统消息中给出了从对局结算截图提取出的信息，请据此自由创作一个战报标题。

创作要求：
- 标题中必须出现信息中的英雄名，英雄名可能是会有前缀“梦”的情况，需要保留；如果没有英雄名则可以不用
- 如果英雄名有错误的需要改正过来，比如“张部”可能指的是“张郃”
- 英雄名必须原样出现，比如梦周瑜不能简化为周瑜，比如吕布不能用别称吕奉先
- 仅依据给出的信息进行创作，不要假设或编造信息里没有的内容
- 切入角度、表达风格、句式结构、英雄名所在位置，全部由你自己决定，不要套用任何固定模板，也不要沿用你的第一反应句式
- 先在脑中快速构思 14 个方向完全不同的标题（不同角度、不同语气如有网感的绝了yyds等等、不同长度、不同修辞手法如押韵比喻等等），再从中挑出最好的那一个，允许口语、玩梗、夸张、古风、悬念、反差、第一人称等任意风格
- 可以结合对局结果、最终比分、KDA 等信息，但并不是必须包含这些信息
- 除了助攻特别优秀的情况，否则不体现助攻
- 标题在10~20字之间
- 巧用标点符号，如「」？！可以增强语气、突出重点、让标题更有视觉冲击力
- 生成的一些方向也可以结合历史典故之类的，比如「三国群雄逐鹿」、「孔融让梨」
- 生成的一些方向也可以生成类似三国演义章节标题那样子又押韵又有意思的

只输出最终那 1 个标题文本，不要编号、不要引号、不要列表、不要任何解释或前后缀文字。`

/** 兼容旧引用：原单模型时代的提示词即现在的「标题创作」提示词 */
export const AI_TITLE_PROMPT = AI_WRITE_PROMPT

/** 稿件列表项（与后端 get_archives 命令返回结构对应） */
export interface ArchiveListItem {
    aid: number
    bvid: string
    title: string
    state: number
    stateDesc: string
    dtime: number
    ptime: number
}

export const useUtilsStore = defineStore('template', () => {
    const archieve_pre = ref<any>(null)
    const topiclist = ref<any[]>([])
    const seasonlist = ref<any[]>([])
    const hasSeason = ref<boolean>(false)

    const common_staff_conf = computed(() => {
        const conf = archieve_pre.value?.common_staff_conf || {}

        const titles = Array.isArray(conf.titles)
            ? [...new Set(conf.titles.map((item: any) => String(item).trim()).filter(Boolean))]
            : []

        const missions = Array.isArray(conf.missions)
            ? [
                  ...new Set(
                      conf.missions
                          .map((item: any) => Number(item))
                          .filter((item: number) => Number.isInteger(item) && item > 0)
                  )
              ]
            : []

        return {
            max_staff: Number(conf.max_staff) > 0 ? Number(conf.max_staff) : 10,
            title_ids: conf.title_ids ?? null,
            titles,
            missions
        }
    })

    const getCurrentVersion = async () => {
        try {
            const version = await invoke('get_current_version')
            return version
        } catch (error) {
            console.error('获取当前版本失败:', error)
            return 'unknown' as string
        }
    }

    const getFileSize = async (filePath: string): Promise<number> => {
        try {
            const size = await invoke<number>('get_file_size', { filePath })
            return size
        } catch (error) {
            console.error('获取文件大小失败:', error)
            throw error
        }
    }

    const readDirRecursive = async (
        dirPath: string,
        includeSubdirs: boolean,
        maxDepth?: number
    ): Promise<Array<{ name: string; path: string; is_directory: boolean }>> => {
        try {
            const files = await invoke<
                Array<{ name: string; path: string; is_directory: boolean }>
            >('read_dir_recursive', {
                dirPath,
                includeSubdirs,
                maxDepth: maxDepth || 20
            })
            return files
        } catch (error) {
            console.error('递归读取目录失败:', error)
            throw error
        }
    }

    const getAvatarCacheDir = async () => {
        try {
            const cacheDir = await invoke<string>('get_avatar_cache_dir')
            return cacheDir
        } catch (error) {
            console.error('获取头像缓存目录失败:', error)
            throw error
        }
    }

    const downloadCover = async (uid: number, url: string) => {
        if (!url) {
            return undefined
        }
        try {
            const cover: string = await invoke('download_cover', { uid, url })
            return 'data:image/jpeg;base64,' + cover
        } catch (error) {
            console.error('下载封面失败:', error)
            throw error
        }
    }

    const initArchievePre = async (uid: number) => {
        try {
            archieve_pre.value = await invoke('get_archive_pre', { uid })
            return archieve_pre
        } catch (error) {
            console.error('获取archieve pre失败:', error)
            throw error
        }
    }

    const initTopicList = async (uid: number) => {
        try {
            topiclist.value = await invoke('get_topic_list', { uid })
            return topiclist
        } catch (error) {
            console.error('获取话题列表失败:', error)
            throw error
        }
    }

    const searchTopics = async (uid: number, query: string) => {
        try {
            const results = await invoke('search_topics', { uid, query })
            return results
        } catch (error) {
            console.error('搜索话题失败:', error)
            throw error
        }
    }

    const searchMention = async (uid: number, keyword?: string) => {
        try {
            const query = (keyword || '').trim()
            const groups = await invoke<MentionUserGroup[]>('search_mention', {
                uid,
                keyword: query || undefined
            })
            return groups || []
        } catch (error) {
            console.error('搜索@用户失败:', error)
            throw error
        }
    }

    const getSeasonList = async (uid: number) => {
        hasSeason.value = false

        try {
            seasonlist.value = ((await invoke('get_season_list', { uid })) as any).seasons
            // {"seasons": [{season_id: 1, section_id: 2, title: '合集1'}, {season_id: 2, section_id: 2, title: '合集2'}]}
            hasSeason.value = seasonlist.value.length > 0
        } catch (error) {
            console.error('获取合集列表失败:', error)
            seasonlist.value = []
            throw error
        }
        return hasSeason.value
    }

    const uploadCover = async (uid: number, file: string) => {
        if (!file) {
            return undefined
        }
        try {
            console.log('上传文件:', file)
            const cover_url: string = await invoke('upload_cover', { uid, file })
            console.log('上传封面成功:', cover_url)
            return cover_url
        } catch (error) {
            console.error('上传封面失败:', error)
            throw error
        }
    }

    /**
     * 步骤一：截取视频倒数第三秒画面并调用「图像识别」模型提取对局信息。
     * @param videoPath 本地视频文件路径
     * @param prompt 识别提示词，默认使用 AI_VISION_PROMPT
     */
    const analyzeVideo = async (
        videoPath: string,
        prompt: string = AI_VISION_PROMPT
    ): Promise<AiAnalyzeResult> => {
        try {
            const result = await invoke<AiAnalyzeResult>('ai_analyze_video', {
                videoPath,
                prompt
            })
            return {
                info: String(result?.info || ''),
                reasoning: String(result?.reasoning || '')
            }
        } catch (error) {
            console.error('AI 图像识别失败:', error)
            throw error
        }
    }

    /**
     * 步骤二：把识别出的对局信息文本交给「标题生成」模型创作标题（纯文本请求）。
     * 标题生成模型开启思考模式时，Rust 端会走 SSE 流式请求，通过 on_reasoning 通道把
     * 「深度思考」过程增量实时回调（每次回调为一段新文本，直接追加展示即可）。
     * @param info 步骤一识别出的对局信息文本（作为上下文）
     * @param prompt 创作提示词，默认使用 AI_WRITE_PROMPT
     * @param onReasoning 思考内容增量回调（可选）
     */
    const generateAiTitle = async (
        info: string,
        prompt: string = AI_WRITE_PROMPT,
        onReasoning?: (delta: string) => void
    ): Promise<AiTitleResult> => {
        const channel = new Channel<AiReasoningEvent>()
        channel.onmessage = payload => {
            if (
                onReasoning &&
                payload &&
                payload.type === 'reasoning' &&
                typeof payload.text === 'string' &&
                payload.text
            ) {
                onReasoning(payload.text)
            }
        }
        try {
            const result = await invoke<AiTitleResult>('generate_ai_title', {
                info,
                prompt,
                onReasoning: channel
            })
            return {
                title: String(result?.title || '').replace(/梦(\*|·)/, '梦'),
                reasoning: String(result?.reasoning || '')
            }
        } catch (error) {
            console.error('AI 生成标题失败:', error)
            throw error
        }
    }

    const getVideoDetail = async (uid: number, videoId: string) => {
        try {
            const detail = await invoke('get_video_detail', { uid, videoId })
            return detail
        } catch (error) {
            console.error('获取视频详情失败:', error)
            throw error
        }
    }

    const getArchives = async (
        uid: number,
        status?: string,
        fromPage?: number,
        maxPages?: number,
        keyword?: string
    ): Promise<ArchiveListItem[]> => {
        try {
            const archives = (await invoke('get_archives', {
                uid,
                status: status ?? 'pubed',
                fromPage: fromPage ?? 1,
                maxPages,
                keyword
            })) as ArchiveListItem[]
            return archives || []
        } catch (error) {
            console.error('获取稿件列表失败:', error)
            throw error
        }
    }

    const getVideoSeason = async (uid: number, aid: number) => {
        if (!hasSeason.value) {
            return 0
        }

        try {
            const season = (await invoke('get_video_season', { uid, aid })) as number
            return season
        } catch (error) {
            console.error('获取视频合集失败:', error)
            throw error
        }
    }

    /**
     * 查询稿件首个分P的 cid
     * 投稿接口只返回 aid/bvid，加入合集需要 cid，新投稿的视频上传阶段拿不到，
     * 因此投稿完成后需要单独查询一次
     */
    const getVideoCid = async (uid: number, aid: number) => {
        try {
            const cid = (await invoke('get_video_cid', { uid, aid })) as number
            return cid
        } catch (error) {
            console.error('获取稿件 cid 失败:', error)
            throw error
        }
    }

    const switchSeason = async (
        uid: number,
        aid: number,
        cid: number,
        seasonId: number,
        sectionId: number,
        title: string,
        add: boolean
    ) => {
        if (!hasSeason.value) {
            return
        }

        try {
            await invoke('switch_season', { uid, aid, cid, seasonId, sectionId, title, add })
        } catch (error) {
            console.error('设置合集失败:', error)
            throw error
        }
    }

    // 消息提示帮助函数
    const showMessage = (
        message: string,
        type: 'success' | 'error' | 'warning' | 'info' = 'info'
    ) => {
        ElMessage({
            message,
            type,
            showClose: true,
            duration: type === 'error' ? 0 : 3000
        })
    }

    // 导出日志
    const exportLogs = async (): Promise<string> => {
        try {
            const result = await invoke<string>('export_logs')
            showMessage('日志导出成功', 'success')
            return result
        } catch (error) {
            console.error('导出日志失败:', error)
            showMessage(`导出日志失败: ${error}`, 'error')
            throw error
        }
    }

    // 检查更新
    const checkUpdate = async (): Promise<string | null> => {
        try {
            const result = await invoke<string | null>('check_update')
            return result
        } catch (error) {
            console.error('检查更新失败:', error)
            showMessage(`检查更新失败: ${error}`, 'error')
            throw error
        }
    }

    // 安全序列化日志参数：
    // - 字符串直接返回
    // - Error 对象提取 name/message/stack，避免序列化成 {}
    // - 其他对象用带「祖先栈」的 replacer，把循环引用（如 Vue 组件实例 component -> vnode）
    //   替换为占位符，避免 JSON.stringify 抛 "Converting circular structure to JSON"
    const safeStringifyLog = (value: any): string => {
        if (typeof value === 'string') return value

        if (value instanceof Error) {
            return JSON.stringify({ name: value.name, message: value.message, stack: value.stack })
        }

        const ancestors: any[] = []
        try {
            return JSON.stringify(value, function (this: any, _key: string, currentValue: any): any {
                if (typeof currentValue === 'bigint') {
                    return String(currentValue)
                }
                if (typeof currentValue !== 'object' || currentValue === null) {
                    return currentValue
                }
                // 祖先栈回退到当前节点的父级（this），再判断当前节点是否已在栈中（即循环引用）
                while (ancestors.length > 0 && ancestors[ancestors.length - 1] !== this) {
                    ancestors.pop()
                }
                if (ancestors.includes(currentValue)) {
                    return '[Circular]'
                }
                ancestors.push(currentValue)
                return currentValue
            })
        } catch (error) {
            return `[序列化失败: ${error}]`
        }
    }

    // 防止日志转发失败时 catch 里的 console.error 再次进入本函数造成递归
    let logForwarding = false

    const log = async (level: string, ...messages: any[]) => {
        if (logForwarding) return
        logForwarding = true
        try {
            await invoke('console_log', {
                level,
                messages: messages.map(msg => safeStringifyLog(msg))
            })
        } catch (error) {
            console.error('日志转发失败:', error)
        } finally {
            logForwarding = false
        }
    }

    return {
        archieve_pre: computed(() => archieve_pre.value),
        typelist: computed<any[]>(() => (archieve_pre.value?.typelist || []) as any[]),
        typeListV2: computed<any[]>(() => (archieve_pre.value?.type_list_v2 || []) as any[]),
        common_staff_conf,
        topiclist: computed(() => topiclist.value),
        seasonlist: computed(() => seasonlist.value),
        getCurrentVersion,
        getFileSize,
        getAvatarCacheDir,
        readDirRecursive,
        uploadCover,
        analyzeVideo,
        generateAiTitle,
        downloadCover,
        initArchievePre,
        initTopicList,
        searchTopics,
        searchMention,
        getVideoDetail,
        getArchives,
        hasSeason,
        getSeasonList,
        getVideoSeason,
        getVideoCid,
        switchSeason,
        showMessage,
        exportLogs,
        checkUpdate,
        log
    }
})
