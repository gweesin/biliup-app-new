import { computed, ref, type ComputedRef, type Ref } from 'vue'
import { ElMessageBox } from 'element-plus'
import { useUploadStore } from '../stores/upload'
import { useUserConfigStore } from '../stores/user_config'
import { useUtilsStore } from '../stores/utils'
import { deleteOriginalVideoFile, deleteOriginalVideoFiles } from '../utils/videoFileCleanup'
import { isVideoReadyForSeparateSubmit } from '../utils/videoSubmit'
import type { SeparateSubmitState, SubmitStatsInput } from '../types/submit'

/** 组合式函数所需的宿主上下文（由 MainView 注入，避免与视图强耦合） */
export interface SeparateSubmitContext {
    selectedUser: Ref<any>
    currentTemplateName: Ref<string>
    submitting: Ref<boolean>
    lastPublishedBadgeRef: Ref<{ refresh?: () => void } | null>
    newTemplateRef: Ref<any>
    loginUsers: ComputedRef<any[]>
    recordSubmitStats: (record: SubmitStatsInput) => void
    autoStartWaitingTasks: () => Promise<void>
    reloadTemplateFromAV: (uid: number, aid: number) => Promise<void>
}

/**
 * 稿件提交相关逻辑：单稿件提交、多稿件（分P逐个提交）提交、自动提交轮询与进度统计
 */
export const useSeparateSubmit = (context: SeparateSubmitContext) => {
    const {
        selectedUser,
        currentTemplateName,
        submitting,
        lastPublishedBadgeRef,
        newTemplateRef,
        loginUsers,
        recordSubmitStats,
        autoStartWaitingTasks,
        reloadTemplateFromAV
    } = context

    const uploadStore = useUploadStore()
    const userConfigStore = useUserConfigStore()
    const utilsStore = useUtilsStore()

    const autoSubmittingRecord = ref<Record<string, boolean>>({})
    const autoSubmitProcessingKeys = ref<Set<string>>(new Set())
    const separateSubmittingRecord = ref<Record<string, boolean>>({})
    const separateSubmitProcessingKeys = ref<Set<string>>(new Set())
    const separateSubmitCancelledKeys = ref<Set<string>>(new Set())
    const separateSubmitStateRecord = ref<Record<string, SeparateSubmitState>>({})
    const submitDispatchRoundRobinCursor = ref(0)

    const lastSubmit = ref<string>('')

    // 生成模板键名
    const getTemplateKey = (uid: number, templateName: string) => `${uid}-${templateName}`

    // 解析模板键名，支持模板名包含 "-"
    const parseTemplateKey = (
        templateKey: string
    ): { uid: number; templateName: string } | null => {
        const separatorIndex = templateKey.indexOf('-')
        if (separatorIndex <= 0) {
            return null
        }

        const uid = Number.parseInt(templateKey.slice(0, separatorIndex), 10)
        const templateName = templateKey.slice(separatorIndex + 1)

        if (Number.isNaN(uid) || !templateName) {
            return null
        }

        return { uid, templateName }
    }

    const currentSeparateTemplateKey = computed(() => {
        if (!selectedUser.value || !currentTemplateName.value) {
            return ''
        }

        return getTemplateKey(selectedUser.value.uid, currentTemplateName.value)
    })

    const separateSubmitting = computed(() => {
        const templateKey = currentSeparateTemplateKey.value
        if (!templateKey) {
            return false
        }

        return Boolean(separateSubmittingRecord.value[templateKey])
    })

    const getOrCreateSeparateSubmitState = (
        uid: number,
        templateName: string
    ): SeparateSubmitState => {
        const templateKey = getTemplateKey(uid, templateName)
        const existing = separateSubmitStateRecord.value[templateKey]
        if (existing) {
            return existing
        }

        const created: SeparateSubmitState = {
            uid,
            templateName,
            attemptedVideoIds: new Set<string>(),
            successCount: 0,
            failCount: 0,
            totalCount: 0,
            successBvids: [],
            failedVideoNames: []
        }

        separateSubmitStateRecord.value[templateKey] = created
        return created
    }

    const clearSeparateSubmitStateByKey = (templateKey: string) => {
        delete separateSubmittingRecord.value[templateKey]
        separateSubmitProcessingKeys.value.delete(templateKey)
        separateSubmitCancelledKeys.value.delete(templateKey)
        delete separateSubmitStateRecord.value[templateKey]
    }

    const clearAllSeparateSubmitStates = () => {
        separateSubmittingRecord.value = {}
        separateSubmitProcessingKeys.value.clear()
        separateSubmitCancelledKeys.value.clear()
        separateSubmitStateRecord.value = {}
    }

    const separateSubmitUploadedCount = computed(() => {
        const templateKey = currentSeparateTemplateKey.value
        if (!templateKey || !separateSubmittingRecord.value[templateKey]) {
            return 0
        }

        const submitState = separateSubmitStateRecord.value[templateKey]
        if (!submitState) {
            return 0
        }

        const { uid, templateName } = submitState
        const targetTemplate = userConfigStore.configRoot?.config?.[uid]?.templates?.[templateName]
        const videos = targetTemplate?.videos || []
        let readyUploadedCount = 0

        for (const video of videos) {
            if (
                video.complete &&
                video.path === '' &&
                !submitState.attemptedVideoIds.has(video.id)
            ) {
                readyUploadedCount++
            }
        }

        const attemptedCount = submitState.attemptedVideoIds.size
        return Math.min(submitState.totalCount, attemptedCount + readyUploadedCount)
    })

    const separateSubmitCompletedCount = computed(() => {
        const templateKey = currentSeparateTemplateKey.value
        if (!templateKey) {
            return 0
        }

        const submitState = separateSubmitStateRecord.value[templateKey]
        if (!submitState) {
            return 0
        }

        return submitState.successCount + submitState.failCount
    })

    const separateSubmitTotalCount = computed(() => {
        const templateKey = currentSeparateTemplateKey.value
        if (!templateKey) {
            return 0
        }

        const submitState = separateSubmitStateRecord.value[templateKey]
        if (!submitState) {
            return 0
        }

        return submitState.totalCount
    })

    // 获取当前模板的自动提交状态
    const getCurrentAutoSubmitting = computed(() => {
        if (!selectedUser.value || !currentTemplateName.value) return false
        const key = getTemplateKey(selectedUser.value.uid, currentTemplateName.value)
        return autoSubmittingRecord.value[key] || false
    })

    // 设置模板的自动提交状态
    const setAutoSubmitting = (uid: number, templateName: string, status: boolean) => {
        const key = getTemplateKey(uid, templateName)
        if (status) {
            autoSubmittingRecord.value[key] = true
        } else {
            delete autoSubmittingRecord.value[key]
        }
    }

    const getSubmitUserLabel = (uid: number) => {
        const user = loginUsers.value.find(item => item.uid === uid)
        if (!user) {
            return '未知用户: ' + uid
        }
        return user.username
    }

    const getSeparateSubmitCancelKey = (uid: number, templateName: string) => {
        return `separate:${uid}:${templateName}`
    }

    // 检查是否有任何模板在自动提交
    const hasAnyAutoSubmitting = computed(() => {
        return Object.keys(autoSubmittingRecord.value).length > 0
    })

    const hasAnySeparateSubmitting = computed(() => {
        return Object.keys(separateSubmittingRecord.value).length > 0
    })

    const getOrderedDispatchTemplateKeys = (templateKeys: string[]) => {
        if (templateKeys.length <= 1) {
            return templateKeys
        }

        const sortedKeys = [...templateKeys].sort()
        const startIndex = submitDispatchRoundRobinCursor.value % sortedKeys.length
        const head = sortedKeys.slice(startIndex)
        const tail = sortedKeys.slice(0, startIndex)
        return [...head, ...tail]
    }

    const syncSeasonAfterSubmit = async (uid: number, resp: any, template: any) => {
        if (!(resp && resp.aid && utilsStore.hasSeason)) {
            return
        }

        const isNewSubmission = !template?.aid
        const configuredSeasonId = Number(template?.season_id || 0)
        // 新增稿件未设置合集时，不触发合集提交
        if (isNewSubmission && configuredSeasonId <= 0) {
            return
        }

        try {
            const old_season_id = await utilsStore.getVideoSeason(uid, resp.aid)
            let add = old_season_id && old_season_id !== 0 ? false : true

            if (template && old_season_id !== template.season_id && template.videos[0]?.cid) {
                const new_season_id = template.season_id || 0
                const new_section_id = template.section_id || 0
                await utilsStore.switchSeason(
                    uid,
                    resp.aid,
                    template.videos[0]?.cid,
                    new_season_id,
                    new_section_id,
                    template.title,
                    add
                )

                const season_title =
                    utilsStore.seasonlist.find((s: any) => s.season_id === template.season_id)
                        ?.title || template.season_id
                utilsStore.showMessage(`视频${resp.bvid}加入合集${season_title}`, 'success')
                console.log(`视频${resp.bvid}加入合集${season_title}`, 'success')
            }
        } catch (error) {
            console.error('设置合集失败: ', error)
            utilsStore.showMessage(`设置合集失败: ${error}`, 'error')
        }
    }

    const handleAutoEditAfterSubmit = async (
        uid: number,
        templateName: string,
        template: any,
        resp: any
    ) => {
        if (!template.aid) {
            const userConfig = userConfigStore.configRoot?.config[uid]
            if (userConfig && userConfig.auto_edit && newTemplateRef.value) {
                // 新增稿件且auto_edit开启，创建编辑模板
                await newTemplateRef.value.createTemplateFromBV(uid, resp.bvid, resp.bvid, true)
                utilsStore.showMessage('从BV号创建模板成功', 'success')
            }
        } else {
            if (selectedUser.value?.uid === uid && currentTemplateName.value === templateName) {
                await reloadTemplateFromAV(uid, template.aid)
            }
        }
    }

    const collectFailedVideoNames = (template: any): string[] => {
        return Array.from(
            new Set(
                (template?.videos || [])
                    .map((video: any) => (video?.title || '').trim() || video?.id)
                    .filter((name: any): name is string => Boolean(name))
            )
        )
    }

    // 执行模板提交
    const performTemplateSubmit = async (
        uid: number,
        templateName: string,
        template: any,
        options?: { showLoading?: boolean }
    ) => {
        const user = loginUsers.value.find(u => u.uid === uid)
        if (!user) throw new Error('用户不存在')

        const showLoading = options?.showLoading ?? true
        if (showLoading) {
            submitting.value = true
        }

        try {
            const resp = (await uploadStore.submitTemplate(uid, template)) as any
            const bvid = resp?.bvid ? String(resp.bvid) : '-'
            recordSubmitStats({
                user: user.username,
                mode: '单稿件',
                templateName,
                status: 'success',
                bvid
            })

            // 更新最后提交时间（只对当前模板）
            if (selectedUser.value?.uid === uid && currentTemplateName.value === templateName) {
                lastSubmit.value = new Date().toLocaleString()
            }

            // 投稿成功后延时刷新已发布稿件列表，更新发布时间展示
            if (selectedUser.value?.uid === uid) {
                setTimeout(() => lastPublishedBadgeRef.value?.refresh?.(), 1500)
            }

            utilsStore.showMessage(`视频${resp.bvid}提交成功 (模板: ${templateName})`, 'success')
            console.log(`视频${resp.bvid}提交成功 (模板: ${templateName})`, 'success')

            // 稿件发布成功后删除模板中所有视频的原始本地文件
            await deleteOriginalVideoFiles(template?.videos)

            await syncSeasonAfterSubmit(uid, resp, template)

            await new Promise(resolve => setTimeout(resolve, 500))

            try {
                await handleAutoEditAfterSubmit(uid, templateName, template, resp)
            } catch (error) {
                utilsStore.showMessage(`${error}`, 'error')
            }
        } catch (error) {
            const failedVideoNames = collectFailedVideoNames(template)
            for (const videoName of failedVideoNames) {
                recordSubmitStats({
                    user: user.username,
                    mode: '单稿件',
                    templateName,
                    status: 'failed',
                    videoName,
                    error
                })
            }

            console.error('模板提交失败:', error)
            utilsStore.showMessage(`模板提交失败: ${error}`, 'error')
        } finally {
            if (showLoading) {
                submitting.value = false
            }
        }
    }

    const finalizeSeparateSubmitMode = async (templateKey: string) => {
        const submitState = separateSubmitStateRecord.value[templateKey]
        if (!submitState) {
            clearSeparateSubmitStateByKey(templateKey)
            return
        }

        const wasCancelled = separateSubmitCancelledKeys.value.has(templateKey)
        if (wasCancelled) {
            const successCount = submitState.successCount
            const failCount = submitState.failCount
            if (successCount > 0 || failCount > 0) {
                utilsStore.showMessage(
                    `多稿件提交已停止，已成功 ${successCount} 个，失败 ${failCount} 个`,
                    'warning'
                )
            }
            clearSeparateSubmitStateByKey(templateKey)
            return
        }

        const targetTemplate =
            userConfigStore.configRoot?.config?.[submitState.uid]?.templates?.[
                submitState.templateName
            ]
        const remainingVideos = targetTemplate?.videos || []
        const hasUploadingVideos = remainingVideos.some(
            video => !(video.complete && video.path === '')
        )
        const hasPendingReadyVideos = remainingVideos.some(
            video =>
                isVideoReadyForSeparateSubmit(video) && !submitState.attemptedVideoIds.has(video.id)
        )
        // 已上传完成但尚未就绪（标题未修改或未设置封面）的视频，等待用户处理后自动继续
        const hasBlockedReadyVideos = remainingVideos.some(
            video =>
                video.complete &&
                video.path === '' &&
                !submitState.attemptedVideoIds.has(video.id) &&
                !isVideoReadyForSeparateSubmit(video)
        )

        // 模板被删光视频后应及时释放状态，避免按钮一直显示“停止多稿件提交”
        if (submitState.totalCount > 0 && remainingVideos.length === 0) {
            const successCount = submitState.successCount
            const failCount = submitState.failCount

            if (failCount === 0) {
                utilsStore.showMessage(`多稿件提交完成，共成功 ${successCount} 个`, 'success')
            } else {
                utilsStore.showMessage(
                    `多稿件提交完成，成功 ${successCount} 个，失败 ${failCount} 个`,
                    'warning'
                )
            }

            clearSeparateSubmitStateByKey(templateKey)
            return
        }

        if (hasUploadingVideos) {
            return
        }

        if (hasPendingReadyVideos) {
            void processSeparateSubmitQueue(templateKey)
            return
        }

        // 存在已上传完成但未就绪（未改名/未设置封面）的视频时保持提交进行中，
        // 待用户修改标题并设置封面后由状态监听触发自动继续
        if (hasBlockedReadyVideos) {
            return
        }

        const successCount = submitState.successCount
        const failCount = submitState.failCount

        if (successCount > 0 || failCount > 0) {
            if (
                selectedUser.value?.uid === submitState.uid &&
                currentTemplateName.value === submitState.templateName
            ) {
                lastSubmit.value = new Date().toLocaleString()
            }
            // 投稿完成后延时刷新已发布稿件列表，更新发布时间展示
            if (successCount > 0) {
                setTimeout(() => lastPublishedBadgeRef.value?.refresh?.(), 1500)
            }
        }

        if (failCount === 0) {
            utilsStore.showMessage(`多稿件提交完成，共成功 ${successCount} 个`, 'success')
        } else {
            utilsStore.showMessage(
                `多稿件提交完成，成功 ${successCount} 个，失败 ${failCount} 个`,
                'warning'
            )
        }

        clearSeparateSubmitStateByKey(templateKey)
    }

    const processSeparateSubmitQueue = async (templateKey: string) => {
        if (
            !separateSubmittingRecord.value[templateKey] ||
            separateSubmitProcessingKeys.value.has(templateKey)
        ) {
            return
        }

        const submitState = separateSubmitStateRecord.value[templateKey]
        if (!submitState) {
            clearSeparateSubmitStateByKey(templateKey)
            return
        }

        separateSubmitProcessingKeys.value.add(templateKey)
        try {
            if (
                !separateSubmittingRecord.value[templateKey] ||
                separateSubmitCancelledKeys.value.has(templateKey)
            ) {
                return
            }

            const { uid, templateName } = submitState
            const targetTemplate =
                userConfigStore.configRoot?.config?.[uid]?.templates?.[templateName]
            if (!targetTemplate) {
                clearSeparateSubmitStateByKey(templateKey)
                utilsStore.showMessage('多稿件提交目标模板不存在，已停止提交', 'warning')
                return
            }

            const readyVideo = (targetTemplate.videos || []).find(
                video =>
                    isVideoReadyForSeparateSubmit(video) &&
                    !submitState.attemptedVideoIds.has(video.id)
            )

            if (!readyVideo) {
                return
            }

            submitState.attemptedVideoIds.add(readyVideo.id)
            const singleVideo = JSON.parse(JSON.stringify(readyVideo))
            const singleTemplate = JSON.parse(JSON.stringify(targetTemplate))
            const fallbackTitle = singleTemplate.title

            singleTemplate.aid = undefined
            singleTemplate.videos = [singleVideo]
            singleTemplate.title = (singleVideo.title || '').trim() || fallbackTitle

            // 如果视频有封面，则使用视频封面，否则使用模板封面
            if (singleVideo.cover) {
                singleTemplate.cover = singleVideo.cover
            }

            // 如果视频有定时发布时间，则使用视频定时发布时间，否则使用模板定时发布时间
            if (singleVideo.dtime) {
                singleTemplate.dtime = singleVideo.dtime
            }
            const cancelKey = getSeparateSubmitCancelKey(uid, templateName)

            try {
                const resp = (await uploadStore.submitTemplate(uid, singleTemplate, {
                    cancelKey
                })) as any
                submitState.successCount++
                if (resp?.bvid) {
                    submitState.successBvids.push(resp.bvid)
                }
                recordSubmitStats({
                    user: getSubmitUserLabel(uid),
                    mode: '多稿件',
                    templateName,
                    status: 'success',
                    bvid: resp?.bvid ? String(resp.bvid) : '-'
                })

                // 稿件发布成功后删除该视频的原始本地文件
                await deleteOriginalVideoFile(readyVideo)

                const removeIndex = targetTemplate.videos.findIndex(v => v.id === readyVideo.id)
                if (removeIndex > -1) {
                    targetTemplate.videos.splice(removeIndex, 1)
                }

                try {
                    await syncSeasonAfterSubmit(uid, resp, singleTemplate)
                    await handleAutoEditAfterSubmit(uid, templateName, singleTemplate, resp)
                } catch (postError) {
                    console.error('提交后处理失败:', postError)
                    utilsStore.showMessage(`提交后处理失败: ${postError}`, 'error')
                }
            } catch (error) {
                if (
                    uploadStore.isSubmitCancelledError(error) ||
                    separateSubmitCancelledKeys.value.has(templateKey)
                ) {
                    return
                }

                submitState.failCount++
                const videoTitle = (readyVideo.title || '').trim() || readyVideo.id
                submitState.failedVideoNames.push(videoTitle)
                recordSubmitStats({
                    user: getSubmitUserLabel(uid),
                    mode: '多稿件',
                    templateName,
                    status: 'failed',
                    videoName: videoTitle,
                    error
                })
                utilsStore.showMessage(`多稿件提交失败（${videoTitle}）: ${error}`, 'error')
                console.error('多稿件模式提交失败: ', error)
            }
        } finally {
            separateSubmitProcessingKeys.value.delete(templateKey)
            void finalizeSeparateSubmitMode(templateKey)
        }
    }

    const stopSeparateSubmit = (templateKey?: string) => {
        const targetKey = templateKey || currentSeparateTemplateKey.value
        if (!targetKey || !separateSubmittingRecord.value[targetKey]) {
            return
        }

        const submitState = separateSubmitStateRecord.value[targetKey]
        if (!submitState) {
            clearSeparateSubmitStateByKey(targetKey)
            return
        }

        const cancelKey = getSeparateSubmitCancelKey(submitState.uid, submitState.templateName)
        uploadStore.cancelPendingSubmitByKey(cancelKey)

        separateSubmitCancelledKeys.value.add(targetKey)
        delete separateSubmittingRecord.value[targetKey]
        utilsStore.showMessage('已停止多稿件提交，当前进行中的提交结束后将退出', 'info')

        if (!separateSubmitProcessingKeys.value.has(targetKey)) {
            void finalizeSeparateSubmitMode(targetKey)
        }
    }

    const submitTemplateAsSeparatePosts = async (options?: {
        skipConfirm?: boolean
        autoTrigger?: boolean
        uid?: number
        templateName?: string
    }) => {
        const submitUid = options?.uid ?? selectedUser.value?.uid
        const submitTemplateName = options?.templateName ?? currentTemplateName.value

        if (!submitUid || !submitTemplateName) {
            utilsStore.showMessage('请先选择模板', 'error')
            return
        }

        const targetTemplate =
            userConfigStore.configRoot?.config?.[submitUid]?.templates?.[submitTemplateName]
        if (!targetTemplate) {
            utilsStore.showMessage('目标模板不存在', 'error')
            return
        }

        const templateKey = getTemplateKey(submitUid, submitTemplateName)
        if (separateSubmittingRecord.value[templateKey]) {
            if (options?.autoTrigger) {
                await processSeparateSubmitQueue(templateKey)
            }
            return
        }

        if (targetTemplate.aid) {
            utilsStore.showMessage('仅新增稿件支持此功能', 'warning')
            return
        }

        const sourceVideos = targetTemplate.videos || []
        if (sourceVideos.length === 0) {
            utilsStore.showMessage('当前没有可提交的视频', 'warning')
            return
        }

        if (!options?.skipConfirm) {
            try {
                await ElMessageBox.confirm(
                    `即将按多稿件模式提交 ${sourceVideos.length} 个视频。每个视频将单独提交为一份稿件；上传完成后需为每个视频修改标题并设置封面，才会自动提交，确认继续吗？`,
                    '确认多稿件提交',
                    {
                        confirmButtonText: '确认提交',
                        cancelButtonText: '取消',
                        type: 'warning'
                    }
                )
            } catch {
                return
            }
        }

        const submitState = getOrCreateSeparateSubmitState(submitUid, submitTemplateName)
        submitState.attemptedVideoIds.clear()
        submitState.successCount = 0
        submitState.failCount = 0
        submitState.totalCount = sourceVideos.length
        submitState.successBvids = []
        submitState.failedVideoNames = []

        separateSubmittingRecord.value[templateKey] = true
        separateSubmitCancelledKeys.value.delete(templateKey)

        try {
            // 确保所有视频都在上传队列中（已存在任务会被自动跳过）
            await uploadStore.createUploadTask(submitUid, submitTemplateName, sourceVideos)

            if (userConfigStore.configRoot?.auto_start) {
                setTimeout(async () => {
                    try {
                        await autoStartWaitingTasks()
                    } catch (error) {
                        console.error('自动开始任务失败:', error)
                    }
                }, 500)
            }

            utilsStore.showMessage('已开启多稿件提交，视频上传完成后将自动逐条提交', 'info')
            await processSeparateSubmitQueue(templateKey)
        } catch (error) {
            console.error('开启多稿件提交失败:', error)
            utilsStore.showMessage(`开启多稿件提交失败: ${error}`, 'error')
            clearSeparateSubmitStateByKey(templateKey)
        }
    }

    // 全局自动提交检查函数
    const checkAutoSubmitAll = async () => {
        const templateKeys = Array.from(
            new Set([
                ...Object.keys(autoSubmittingRecord.value),
                ...Object.keys(separateSubmittingRecord.value)
            ])
        )

        if (templateKeys.length === 0) {
            return
        }

        const orderedTemplateKeys = getOrderedDispatchTemplateKeys(templateKeys)
        submitDispatchRoundRobinCursor.value =
            (submitDispatchRoundRobinCursor.value + 1) % orderedTemplateKeys.length

        for (const templateKey of orderedTemplateKeys) {
            const parsed = parseTemplateKey(templateKey)
            if (!parsed) continue

            const { uid, templateName } = parsed

            // 获取用户和模板配置
            const user = loginUsers.value.find(u => u.uid === uid)
            if (!user || !userConfigStore.configRoot?.config[uid]?.templates[templateName]) {
                // 如果用户或模板不存在，清除自动提交状态
                setAutoSubmitting(uid, templateName, false)
                clearSeparateSubmitStateByKey(templateKey)
                continue
            }

            if (separateSubmittingRecord.value[templateKey]) {
                await processSeparateSubmitQueue(templateKey)
            }

            if (autoSubmitProcessingKeys.value.has(templateKey)) {
                continue
            }

            const template = userConfigStore.configRoot.config[uid].templates[templateName]

            // 检查是否所有文件都已上传完成
            if (template.videos && template.videos.length > 0) {
                const allUploaded = template.videos.every(
                    video => video.complete && video.path === ''
                )

                if (allUploaded && autoSubmittingRecord.value[templateKey]) {
                    // 文件已全部上传完成，执行提交
                    autoSubmitProcessingKeys.value.add(templateKey)
                    setAutoSubmitting(uid, templateName, false)
                    try {
                        await performTemplateSubmit(uid, templateName, template, {
                            showLoading: false
                        })
                    } catch (error) {
                        console.error(`模板 ${templateKey} 自动提交失败:`, error)
                    } finally {
                        autoSubmitProcessingKeys.value.delete(templateKey)
                    }
                }
            } else {
                // 没有视频文件，清除自动提交状态
                setAutoSubmitting(uid, templateName, false)
            }
        }
    }

    // 重置全部自动提交 / 多稿件提交状态（组件卸载、用户登出等场景）
    const resetAllSubmitStates = () => {
        clearAllSeparateSubmitStates()
        autoSubmitProcessingKeys.value.clear()
        autoSubmittingRecord.value = {}
    }

    return {
        // 状态
        autoSubmittingRecord,
        separateSubmittingRecord,
        lastSubmit,
        // 进度与状态计算属性
        currentSeparateTemplateKey,
        separateSubmitting,
        separateSubmitUploadedCount,
        separateSubmitCompletedCount,
        separateSubmitTotalCount,
        getCurrentAutoSubmitting,
        hasAnyAutoSubmitting,
        hasAnySeparateSubmitting,
        // 工具方法
        getTemplateKey,
        parseTemplateKey,
        setAutoSubmitting,
        clearSeparateSubmitStateByKey,
        clearAllSeparateSubmitStates,
        resetAllSubmitStates,
        // 提交流程
        checkAutoSubmitAll,
        performTemplateSubmit,
        stopSeparateSubmit,
        finalizeSeparateSubmitMode,
        processSeparateSubmitQueue,
        submitTemplateAsSeparatePosts
    }
}
