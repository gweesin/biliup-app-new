import { ElMessageBox } from 'element-plus'
import { ref, type Ref } from 'vue'
import { v4 as uuidv4 } from 'uuid'
import { open } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
import { useUploadStore } from '../stores/upload'
import { useUserConfigStore } from '../stores/user_config'
import { useUtilsStore } from '../stores/utils'

/** 支持的视频扩展名 */
const VIDEO_EXTENSIONS = [
    'mp4',
    'flv',
    'avi',
    'wmv',
    'mov',
    'webm',
    'mpeg4',
    'ts',
    'mpg',
    'rm',
    'rmvb',
    'mkv',
    'm4v'
]

/** 单次提交可添加的视频数量上限 */
const MAX_VIDEOS_PER_SUBMIT = 100

export interface VideoImportContext {
    currentForm: Ref<any>
    selectedUser: Ref<any>
    currentTemplateName: Ref<string>
    templateLoading: Ref<boolean>
    uploading: Ref<boolean>
    autoStartWaitingTasks: () => Promise<void>
}

/**
 * 视频文件的导入与管理：文件选择、拖拽导入、队列任务取消、清空与删除
 */
export const useVideoImport = (context: VideoImportContext) => {
    const {
        currentForm,
        selectedUser,
        currentTemplateName,
        templateLoading,
        uploading,
        autoStartWaitingTasks
    } = context

    const uploadStore = useUploadStore()
    const userConfigStore = useUserConfigStore()
    const utilsStore = useUtilsStore()

    const isDragOver = ref(false)

    // 标题为空时，自动使用本次导入的第一个视频文件名（去扩展名）作为标题
    const ensureTitleFromFirstVideo = (videoTitle: string) => {
        if (!currentForm.value) return

        const currentTitle = (currentForm.value.title || '').trim()
        if (currentTitle) return

        const importedVideoTitle = (videoTitle || '').trim()
        if (importedVideoTitle) {
            currentForm.value.title = importedVideoTitle
        }
    }

    // 延迟触发自动开始上传，给队列留出写入时间
    const scheduleAutoStart = () => {
        setTimeout(async () => {
            try {
                await autoStartWaitingTasks()
            } catch (error) {
                console.error('自动开始任务失败:', error)
            }
        }, 500)
    }

    const addVideoToCurrentForm = async (videoPath: string) => {
        // 从路径中提取文件名
        const videoBaseName = videoPath.split(/[/\\]/).pop() || videoPath
        const videoNameWOExtension = videoBaseName.replace(/\.[^/.]+$/, '').slice(0, 80)
        const videoExt = videoBaseName.split('.').pop()?.toLowerCase() || ''

        if (videoExt && !VIDEO_EXTENSIONS.includes(videoExt)) {
            return 0 // 不支持的格式，跳过添加
        }

        // 检查文件是否已经存在
        if (!currentForm.value) {
            return 0 // 没有当前模板，跳过添加
        }

        const existingFile = currentForm.value.videos.find(
            (f: any) => f.path === videoPath || videoNameWOExtension === f.title
        )
        if (existingFile) {
            return 0 // 跳过已存在的文件
        }

        const currentAddedVideos = currentForm.value.videos.filter((video: any) => {
            return (
                (video.finished_at && video.finished_at > 0) ||
                (video.path && video.path.trim() !== '')
            )
        })

        // 检查是否超过100个视频的限制
        if (currentAddedVideos.length >= MAX_VIDEOS_PER_SUBMIT) {
            utilsStore.showMessage('单次提交最大限制100个视频文件，无法添加更多视频', 'error')
            return 0
        }

        // 添加到currentForm.videos
        const videoId = uuidv4()
        currentForm.value.videos.push({
            id: videoId,
            filename: videoBaseName, // 使用完整的文件路径
            title: videoNameWOExtension, // 去除扩展名作为标题
            desc: '',
            path: videoPath, // 保存完整路径
            original_file_path: videoPath, // 原始文件路径，稿件发布成功后用于删除
            complete: false
        })

        // 标题为空时，自动使用本次导入的第一个视频文件名（去扩展名）作为标题
        ensureTitleFromFirstVideo(videoNameWOExtension)

        // 检查是否启用自动添加到上传队列
        if (userConfigStore.configRoot?.auto_upload && selectedUser.value) {
            try {
                // 自动创建上传任务
                await uploadStore.createUploadTask(
                    selectedUser.value.uid,
                    currentTemplateName.value,
                    currentForm.value.videos
                )
                console.log(`自动添加文件到上传队列: ${videoBaseName}`)

                // 如果同时启用自动开始，则自动开始任务
                if (userConfigStore.configRoot?.auto_start) {
                    scheduleAutoStart()
                }
            } catch (error) {
                console.error('自动添加到上传队列失败:', error)
            }
        }
        return 1
    }

    // 处理拖拽文件
    const handleDroppedFiles = async (videoFiles: any) => {
        // 检查是否有选中的用户和模板
        if (!selectedUser.value || !currentTemplateName.value) {
            utilsStore.showMessage('请先选择用户和模板后再拖拽文件', 'warning')
            return
        }

        // 添加视频文件到当前模板
        let addedCount = 0
        templateLoading.value = true
        for (const videoPath of videoFiles.paths) {
            addedCount += await addVideoToCurrentForm(videoPath)
        }
        templateLoading.value = false

        if (addedCount > 0) {
            utilsStore.showMessage(`成功添加 ${addedCount} 个视频文件`, 'success')
        } else {
            utilsStore.showMessage('所有文件都已存在，未添加新文件', 'info')
        }
    }

    // 设置拖拽功能
    const setupDragAndDrop = async () => {
        try {
            // 监听文件拖拽事件
            await listen('tauri://drag-drop', async event => {
                const videos = event.payload as string[]
                isDragOver.value = false
                if (templateLoading.value) {
                    utilsStore.showMessage('模板加载中', 'warning')
                    return
                }
                await handleDroppedFiles(videos)
            })

            // 监听拖拽悬停事件
            await listen('tauri://drag-over', event => {
                if (!isDragOver.value) {
                    console.log('文件拖拽悬停:', event.payload, '，忽略后续日志')
                }
                isDragOver.value = true
            })

            // 监听拖拽取消事件
            await listen('tauri://drag-leave', () => {
                console.log('文件拖拽取消')
                isDragOver.value = false
            })
        } catch (error) {
            console.error('设置拖拽功能失败: ', error)
            utilsStore.showMessage(`'设置拖拽功能失败: ${error}'`, 'error')
        }
    }

    const selectVideoWithTauri = async () => {
        if (templateLoading.value) {
            utilsStore.showMessage('模板加载中', 'warning')
            return
        }

        templateLoading.value = true
        try {
            const selected = await open({
                multiple: true,
                filters: [
                    {
                        name: 'Video',
                        extensions: [...VIDEO_EXTENSIONS]
                    }
                ]
            })

            let added = 0

            if (selected && Array.isArray(selected)) {
                for (const videoPath of selected) {
                    added += await addVideoToCurrentForm(videoPath)
                }

                utilsStore.showMessage(`已选择 ${added} 个文件`, 'success')
            } else if (typeof selected === 'string') {
                added += await addVideoToCurrentForm(selected)
                utilsStore.showMessage(`已选择 ${added} 个文件`, 'success')
            }
        } catch (error) {
            console.error('文件选择失败: ', error)
            utilsStore.showMessage(`'文件选择失败: ${error}'`, 'error')
        } finally {
            templateLoading.value = false
        }
    }

    // 清空所有文件
    const clearAllVideos = async () => {
        if (!currentForm.value?.videos || currentForm.value.videos.length === 0) {
            return
        }

        const videoCount = currentForm.value.videos.length
        const videoText = videoCount === 1 ? '1 个文件' : `${videoCount} 个文件`

        templateLoading.value = true
        try {
            await ElMessageBox.confirm(`确定要清空所有已选择的 ${videoText} 吗？`, '确认清空文件', {
                confirmButtonText: '确定清空',
                cancelButtonText: '取消',
                type: 'warning',
                dangerouslyUseHTMLString: false
            })

            // 取消所有对应的上传任务
            const videoIds = currentForm.value.videos.map((video: any) => video.id)
            const correspondingTasks = uploadStore.uploadQueue.filter(task =>
                videoIds.includes(task.video?.id)
            )

            for (const task of correspondingTasks) {
                try {
                    await uploadStore.cancelUpload(task.id)
                    console.log(`已取消对应的上传任务: ${task.id}`)
                } catch (error) {
                    console.error('取消上传任务失败:', error)
                    // 继续处理其他任务
                }
            }

            // 清空视频文件列表
            currentForm.value.videos = []
            utilsStore.showMessage(`已清空 ${videoText}`, 'success')
        } catch {
            // 用户取消了操作
        } finally {
            templateLoading.value = false
        }
    }

    // 删除单个视频（同时取消其上传任务）
    const removeUploadedFile = async (videoId: string) => {
        if (!currentForm.value?.videos) {
            return
        }

        templateLoading.value = true
        const videoIndex = currentForm.value.videos.findIndex((f: any) => f.id === videoId)
        if (videoIndex > -1) {
            const video = currentForm.value.videos[videoIndex]

            try {
                // 添加确认弹窗
                await ElMessageBox.confirm(
                    `确定要删除视频文件"${video.title}"吗？此操作不可撤销。`,
                    '确认删除文件',
                    {
                        confirmButtonText: '确定删除',
                        cancelButtonText: '取消',
                        type: 'warning'
                    }
                )

                // 先查找并取消对应的上传任务
                const correspondingTask = uploadStore.uploadQueue.find(
                    task => task.video?.id === videoId
                )
                if (correspondingTask) {
                    try {
                        await uploadStore.cancelUpload(correspondingTask.id)
                        console.log(`已取消对应的上传任务: ${correspondingTask.id}`)
                    } catch (error) {
                        console.error('取消上传任务失败:', error)
                        // 即使取消失败，仍然继续删除文件
                    }
                }

                // 删除视频文件
                currentForm.value.videos.splice(videoIndex, 1)

                utilsStore.showMessage('文件删除成功', 'success')
            } catch (error) {
                // 如果用户取消了确认框，不显示错误消息
                if (error !== 'cancel') {
                    console.error('删除文件失败:', error)
                    utilsStore.showMessage(`删除文件失败: ${error}`, 'error')
                }
            }
        }
        templateLoading.value = false
    }

    // 将当前模板的视频加入上传队列
    const createUpload = async () => {
        // 检查是否有文件可上传
        const hasUploadedFiles = currentForm.value?.videos && currentForm.value.videos.length > 0

        if (!hasUploadedFiles) {
            utilsStore.showMessage('请先选择视频文件', 'error')
            return
        }

        if (!selectedUser.value) {
            utilsStore.showMessage('请先选择用户', 'error')
            return
        }

        uploading.value = true
        try {
            if (currentForm.value) {
                console.log('开始上传文件:', currentForm.value.videos)
                // 确保传递的是正确格式的数组
                const num_added = await uploadStore.createUploadTask(
                    selectedUser.value.uid,
                    currentTemplateName.value,
                    currentForm.value.videos
                )
                utilsStore.showMessage(`添加 ${num_added} 个文件到上传队列`, 'success')
            }

            // 如果启用自动开始，则自动开始任务
            if (userConfigStore.configRoot?.auto_start) {
                scheduleAutoStart()
            }
        } catch (error) {
            console.error('上传失败: ', error)
            utilsStore.showMessage(`上传失败: ${error}`, 'error')
        } finally {
            uploading.value = false
        }
    }

    // 处理文件夹监控添加视频事件
    const handleAddVideosToForm = async (newVideos: any[]) => {
        templateLoading.value = true
        for (const videoPath of newVideos) {
            try {
                await addVideoToCurrentForm(videoPath)
            } catch (error) {
                console.error(`添加视频失败: ${videoPath}`, error)
            }
        }
        templateLoading.value = false
    }

    return {
        isDragOver,
        setupDragAndDrop,
        addVideoToCurrentForm,
        handleDroppedFiles,
        selectVideoWithTauri,
        clearAllVideos,
        removeUploadedFile,
        createUpload,
        handleAddVideosToForm
    }
}
