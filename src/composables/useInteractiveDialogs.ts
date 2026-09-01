import { ref, watch, type Ref } from 'vue'
import { ElMessageBox } from 'element-plus'

export interface InteractiveDialogContext {
    currentForm: Ref<any>
    selectedUser: Ref<any>
    templateLoading: Ref<boolean>
}

/**
 * 互动视频 / 仅自己可见 的二次确认弹窗逻辑：
 * 在切换开关时弹出说明与确认，并支持"不再提示"记忆。
 */
export const useInteractiveDialogs = (context: InteractiveDialogContext) => {
    const { currentForm, selectedUser, templateLoading } = context

    // 互动视频确认记录（key 为当前用户 uid）
    const interactiveConfirmShown = ref<Record<string, boolean>>({})
    // 互动视频确认框防重入标志
    const interactiveDialogOpening = ref(false)
    // 仅自己可见确认框防重入标志
    const onlySelfDialogOpening = ref(false)

    const getInteractiveConfirmKey = () => {
        if (!selectedUser.value?.uid) return ''
        return `${selectedUser.value.uid}`
    }

    const showInteractiveInfoDialog = async () => {
        await ElMessageBox.alert(
            '勾选后本视频将被投稿为互动视频，需在规定时间内完成剧情树配置，否则系统可能回收稿件。',
            '互动功能说明',
            {
                confirmButtonText: '知道了',
                type: 'warning'
            }
        )
    }

    const confirmInteractiveEnable = async () => {
        let dontShowAgain = false

        await ElMessageBox.confirm(
            '<div class="interactive-confirm-dialog">' +
                '<div class="interactive-confirm-dialog-text">互动视频需在规定时间内完成剧情树配置，否则系统可能回收稿件。</div>' +
                '<label class="interactive-confirm-dialog-checkbox">' +
                '<input id="interactive-dont-show-again" type="checkbox" />' +
                '<span>以后不再显示提示</span>' +
                '</label>' +
                '</div>',
            '确认',
            {
                confirmButtonText: '是，我已知晓',
                cancelButtonText: '否',
                type: 'warning',
                dangerouslyUseHTMLString: true,
                beforeClose: (action, _instance, done) => {
                    void _instance
                    if (action === 'confirm') {
                        const checkbox = document.getElementById(
                            'interactive-dont-show-again'
                        ) as HTMLInputElement | null
                        dontShowAgain = Boolean(checkbox?.checked)
                    }
                    done()
                }
            }
        )

        return dontShowAgain
    }

    // 监听互动视频开关变化，弹出确认
    watch(
        () => currentForm.value?.interactive,
        async (newValue, oldValue) => {
            if (!currentForm.value || interactiveDialogOpening.value || templateLoading.value)
                return
            if (newValue !== 1 || oldValue === 1) return

            const key = getInteractiveConfirmKey()
            if (!key || interactiveConfirmShown.value[key]) return

            interactiveDialogOpening.value = true
            try {
                const dontShowAgain = await confirmInteractiveEnable()
                if (dontShowAgain) {
                    interactiveConfirmShown.value[key] = true
                }
            } catch {
                currentForm.value.interactive = 0
            } finally {
                interactiveDialogOpening.value = false
            }
        }
    )

    // 监听"仅自己可见"开关变化，联合投稿时弹出确认
    watch(
        () => currentForm.value?.is_only_self,
        async (newValue, oldValue) => {
            if (!currentForm.value || onlySelfDialogOpening.value || templateLoading.value) return
            if (Number(newValue || 0) !== 1 || Number(oldValue || 0) === 1) return

            const staffList = Array.isArray(currentForm.value.staff) ? currentForm.value.staff : []
            if (staffList.length === 0) return

            onlySelfDialogOpening.value = true
            try {
                await ElMessageBox.confirm(
                    '仅自己可见时无法进行联合投稿，如果继续，将清空联合投稿信息，是否继续。',
                    '提示',
                    {
                        confirmButtonText: '继续',
                        cancelButtonText: '取消',
                        type: 'warning'
                    }
                )
                currentForm.value.staff = []
            } catch {
                currentForm.value.is_only_self = 0
            } finally {
                onlySelfDialogOpening.value = false
            }
        }
    )

    return {
        interactiveConfirmShown,
        interactiveDialogOpening,
        onlySelfDialogOpening,
        getInteractiveConfirmKey,
        showInteractiveInfoDialog
    }
}
