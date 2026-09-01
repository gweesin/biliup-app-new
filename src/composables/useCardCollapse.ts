import { ref } from 'vue'

export type CardCollapsedKey = 'basic' | 'tags' | 'description' | 'videos' | 'advanced'

const CARD_COLLAPSED_KEY = 'card-collapsed-state'

/**
 * 卡片折叠状态及其本地持久化
 */
export const useCardCollapse = () => {
    const cardCollapsed = ref<Record<CardCollapsedKey, boolean>>({
        basic: false, // 基本信息
        tags: false, // 标签设置
        description: false, // 视频描述
        videos: false, // 视频文件
        advanced: false // 高级选项
    })

    // 保存卡片折叠状态
    const saveCardCollapsedState = () => {
        try {
            localStorage.setItem(CARD_COLLAPSED_KEY, JSON.stringify(cardCollapsed.value))
        } catch (error) {
            console.error('保存卡片折叠状态失败:', error)
        }
    }

    // 恢复卡片折叠状态
    const restoreCardCollapsedState = () => {
        try {
            const saved = localStorage.getItem(CARD_COLLAPSED_KEY)
            if (saved) {
                const savedState = JSON.parse(saved)
                Object.assign(cardCollapsed.value, savedState)
            }
        } catch (error) {
            console.error('恢复卡片折叠状态失败:', error)
        }
    }

    // 切换卡片折叠状态
    const toggleCardCollapsed = (cardKey: CardCollapsedKey) => {
        cardCollapsed.value[cardKey] = !cardCollapsed.value[cardKey]
        saveCardCollapsedState()
    }

    return {
        cardCollapsed,
        toggleCardCollapsed,
        saveCardCollapsedState,
        restoreCardCollapsedState
    }
}
