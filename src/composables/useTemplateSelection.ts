import { ref, type ComputedRef, type Ref } from 'vue'

const TEMPLATE_SELECTION_KEY = 'last-selected-template'

/** 模板选择记录的有效期（超过后自动失效） */
const SELECTION_MAX_AGE_MS = 30 * 24 * 60 * 60 * 1000

interface RestoreResult {
    userUid: number
    templateName: string
}

export interface TemplateSelectionContext {
    loginUsers: ComputedRef<any[]>
    userTemplates: ComputedRef<any[]>
    selectTemplate: (user: any, templateName: string) => Promise<void>
    toggleUserExpanded: (userUid: number) => void
    showMessage: (message: string, type?: 'success' | 'error' | 'warning' | 'info') => void
    isRestoringTemplate?: Ref<boolean>
}

/**
 * 模板选择的本地记忆：保存上次选择的模板并在启动时恢复
 */
export const useTemplateSelection = (context: TemplateSelectionContext) => {
    const { loginUsers, userTemplates, selectTemplate, toggleUserExpanded, showMessage } = context

    // 是否正在恢复模板选择（避免递归保存）
    const isRestoringTemplate = context.isRestoringTemplate ?? ref(false)

    // 保存模板选择到localStorage
    const saveTemplateSelection = (userUid: number, templateName: string) => {
        // 如果正在恢复模板，不保存（避免递归）
        if (isRestoringTemplate.value) return

        try {
            const selection = {
                userUid,
                templateName,
                timestamp: Date.now()
            }
            localStorage.setItem(TEMPLATE_SELECTION_KEY, JSON.stringify(selection))
        } catch (error) {
            console.error('保存模板选择失败:', error)
        }
    }

    // 清除已保存的选择记录
    const clearSavedSelection = () => {
        localStorage.removeItem(TEMPLATE_SELECTION_KEY)
    }

    // 仅当记录属于指定用户时清除（用于用户登出）
    const clearSavedSelectionForUser = (uid: number) => {
        try {
            const saved = localStorage.getItem(TEMPLATE_SELECTION_KEY)
            if (!saved) return

            const selection = JSON.parse(saved)
            if (selection.userUid === uid) {
                clearSavedSelection()
            }
        } catch (error) {
            console.error('清理localStorage记录失败:', error)
        }
    }

    // 读取并校验本地保存的模板选择
    const readValidSelection = (): RestoreResult | null => {
        const saved = localStorage.getItem(TEMPLATE_SELECTION_KEY)
        if (!saved) return null

        try {
            const selection = JSON.parse(saved)
            const { userUid, templateName, timestamp } = selection

            // 检查数据有效性（超过30天的记录自动失效）
            if (timestamp && timestamp < Date.now() - SELECTION_MAX_AGE_MS) {
                clearSavedSelection()
                return null
            }

            // 检查用户是否仍然登录
            const targetUser = loginUsers.value.find(user => user.uid === userUid)
            if (!targetUser) {
                clearSavedSelection()
                return null
            }

            if (targetUser.expired) {
                // 过期账号不恢复历史模板选择
                clearSavedSelection()
                return null
            }

            // 检查模板是否仍然存在
            const userTemplate = userTemplates.value.find(ut => ut.user.uid === userUid)
            const template = userTemplate?.templates.find((t: any) => t.name === templateName)
            if (!template) {
                clearSavedSelection()
                return null
            }

            // 确保用户是展开状态
            if (userTemplate && !userTemplate.expanded) {
                toggleUserExpanded(userUid)
            }

            return { userUid, templateName }
        } catch (error) {
            console.error('解析模板选择记录失败:', error)
            clearSavedSelection()
            return null
        }
    }

    // 从localStorage恢复模板选择
    const restoreTemplateSelection = async () => {
        let selection: RestoreResult | null = null

        try {
            selection = readValidSelection()
            if (!selection) return

            const { userUid, templateName } = selection
            const targetUser = loginUsers.value.find(user => user.uid === userUid)

            // 设置恢复状态标志
            isRestoringTemplate.value = true

            // 自动选择模板
            await selectTemplate(targetUser, templateName)

            console.log(`自动恢复模板选择: ${targetUser.username} - ${templateName}`)
            showMessage(`已恢复上次选择的模板: ${templateName}`, 'success')
        } catch (error) {
            console.error('恢复模板选择失败:', error)
            // 如果恢复失败，清除无效的存储数据
            clearSavedSelection()
        } finally {
            // 确保标志被重置
            isRestoringTemplate.value = false
        }
    }

    return {
        isRestoringTemplate,
        saveTemplateSelection,
        restoreTemplateSelection,
        clearSavedSelection,
        clearSavedSelectionForUser
    }
}
