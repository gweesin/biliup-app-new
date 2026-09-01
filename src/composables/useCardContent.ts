import { ElMessageBox } from 'element-plus'
import type { Ref } from 'vue'

export type ClearableCardType = 'basic' | 'tags' | 'description' | 'advanced'

export interface CardContentContext {
    currentForm: Ref<any>
    tags: Ref<string[]>
    tagViewRef: Ref<{ clearTags?: () => void } | null>
    selectedCategory: Ref<any>
    selectedSubCategory: Ref<any>
    templateLoading: Ref<boolean>
    selectedUser: Ref<any>
    /** 读取用户配置中的默认水印设置 */
    getUserWatermark: (uid: number) => number
    showMessage: (message: string, type?: 'success' | 'error' | 'warning' | 'info') => void
}

/** 卡片显示名称映射 */
const CARD_DISPLAY_NAMES: Record<string, string> = {
    basic: '基本信息',
    tags: '标签设置',
    description: '视频描述',
    videos: '视频文件',
    advanced: '高级选项'
}

/**
 * 表单卡片的显示名称与「清空内容」操作
 */
export const useCardContent = (context: CardContentContext) => {
    const {
        currentForm,
        tags,
        tagViewRef,
        selectedCategory,
        selectedSubCategory,
        templateLoading,
        selectedUser,
        getUserWatermark,
        showMessage
    } = context

    // 获取卡片显示名称
    const getCardDisplayName = (cardType: string): string => {
        return CARD_DISPLAY_NAMES[cardType] || cardType
    }

    // 清空卡片内容
    const clearCardContent = async (cardType: ClearableCardType) => {
        if (!currentForm.value) {
            showMessage('请先选择模板', 'warning')
            return
        }

        // 如果正在加载模板，禁止清空
        if (templateLoading.value) {
            showMessage('模板正在加载中，请稍后再试', 'warning')
            return
        }

        try {
            // 确认清空
            await ElMessageBox.confirm(
                `确定要清空"${getCardDisplayName(cardType)}"的所有内容吗？`,
                '确认清空',
                {
                    confirmButtonText: '确定',
                    cancelButtonText: '取消',
                    type: 'warning'
                }
            )

            // 根据卡片类型清空相应内容
            switch (cardType) {
                case 'basic':
                    currentForm.value.title = ''
                    currentForm.value.cover = ''
                    currentForm.value.tid = 0
                    currentForm.value.tid_v2 = 0
                    currentForm.value.copyright = 1
                    currentForm.value.source = ''
                    // 同步清空分区选择状态
                    selectedCategory.value = null
                    selectedSubCategory.value = null
                    break

                case 'tags':
                    currentForm.value.tag = ''
                    // 同步清空标签数组
                    tags.value = []
                    // 通过组件引用清空TagView的状态
                    tagViewRef.value?.clearTags?.()
                    currentForm.value.staff = undefined
                    break

                case 'description':
                    currentForm.value.desc = ''
                    currentForm.value.desc_v2 = undefined
                    currentForm.value.dynamic = ''
                    break

                case 'advanced':
                    currentForm.value.watermark = getUserWatermark(selectedUser.value?.uid)
                    currentForm.value.dtime = undefined
                    currentForm.value.interactive = 0
                    currentForm.value.dolby = 0
                    currentForm.value.lossless_music = 0
                    currentForm.value.no_reprint = 0
                    currentForm.value.open_elec = 0
                    currentForm.value.no_disturbance = 0
                    currentForm.value.up_selection_reply = 0
                    currentForm.value.up_close_reply = 0
                    currentForm.value.up_close_danmu = 0
                    currentForm.value.atomic_int = 0
                    currentForm.value.is_only_self = 0
                    currentForm.value.is_360 = -1
                    break
            }

            showMessage(`已清空"${getCardDisplayName(cardType)}"的内容`, 'success')
        } catch (error) {
            // 用户取消了操作
        }
    }

    return {
        getCardDisplayName,
        clearCardContent
    }
}
