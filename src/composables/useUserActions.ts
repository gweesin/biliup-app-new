import { ref, type Ref } from 'vue'
import { syncCompletedTasksToConfig } from '../utils/videoFileCleanup'
import { useAuthStore } from '../stores/auth'
import { useUploadStore } from '../stores/upload'
import { useUserConfigStore } from '../stores/user_config'
import { useUtilsStore } from '../stores/utils'

export interface UserActionsContext {
    selectedUser: Ref<any>
    currentTemplateName: Ref<string>
    showLoginDialog: Ref<boolean>
    refreshAllData: () => Promise<void>
    clearSavedSelection: () => void
    clearSavedSelectionForUser: (uid: number) => void
}

/**
 * 用户维度操作：用户配置弹窗、登出、上传任务归属判断
 * 以及上传队列轮询辅助函数。
 */
export const useUserActions = (context: UserActionsContext) => {
    const {
        selectedUser,
        currentTemplateName,
        showLoginDialog,
        refreshAllData,
        clearSavedSelection,
        clearSavedSelectionForUser
    } = context

    const authStore = useAuthStore()
    const uploadStore = useUploadStore()
    const userConfigStore = useUserConfigStore()
    const utilsStore = useUtilsStore()

    // 用户配置弹窗状态
    const userConfigVisible = ref(false)
    const configUser = ref<any>(null)

    // 判断上传队列中是否有需要轮询的活跃任务
    const hasActiveUploadTasks = () => {
        return uploadStore.uploadQueue.some(
            task =>
                task.status === 'Running' || task.status === 'Waiting' || task.status === 'Pending'
        )
    }

    // 将已完成任务的文件信息同步到模板配置
    const syncCompletedTasksFromQueue = () => {
        return syncCompletedTasksToConfig(userConfigStore.configRoot, uploadStore.uploadQueue)
    }

    // 打开用户配置
    const openUserConfig = (user: any) => {
        if (user?.expired) {
            showLoginDialog.value = true
            utilsStore.showMessage('该用户登录状态已过期，请重新登录', 'warning')
            return
        }

        configUser.value = user
        userConfigVisible.value = true
    }

    // 检查用户是否有上传任务
    const isUserHasUploadTasks = (uid: number) => {
        return uploadStore.uploadQueue.some((task: any) => task.user?.uid === uid)
    }

    // 处理用户登出
    const handleLogoutUser = async (uid: number) => {
        // 如果用户有上传任务，不允许登出
        if (isUserHasUploadTasks(uid)) {
            utilsStore.showMessage('用户有未完成的上传任务，无法登出', 'success')
            return
        }

        try {
            const success = await authStore.logoutUser(uid)
            if (success) {
                // 如果登出的用户正是当前选择的用户，清除相关记录
                if (selectedUser.value?.uid === uid) {
                    selectedUser.value = null
                    currentTemplateName.value = ''
                    clearSavedSelection()
                } else {
                    // 检查localStorage中记录的用户是否是被登出的用户
                    clearSavedSelectionForUser(uid)
                }

                utilsStore.showMessage('用户已登出', 'success')
                // 刷新前端数据
                await refreshAllData()
            } else {
                utilsStore.showMessage('登出失败', 'error')
            }
        } catch (error) {
            // 如果用户取消了确认框，error会是'cancel'，不需要显示错误
            if (error !== 'cancel') {
                console.error('登出用户失败:', error)
                utilsStore.showMessage(`登出失败: ${error}`, 'error')
            }
        }
    }

    return {
        userConfigVisible,
        configUser,
        hasActiveUploadTasks,
        syncCompletedTasksFromQueue,
        openUserConfig,
        isUserHasUploadTasks,
        handleLogoutUser
    }
}
