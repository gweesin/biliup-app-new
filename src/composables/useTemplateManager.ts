import { computed, nextTick, ref, watch, type ComputedRef, type Ref } from 'vue'
import { ElMessageBox } from 'element-plus'
import { hasUnsavedChanges } from '../utils/templateDiff'
import { useUserConfigStore } from '../stores/user_config'
import { useUtilsStore } from '../stores/utils'

export interface TemplateManagerContext {
    templateLoading: Ref<boolean>
    selectedUser: Ref<any>
    currentTemplateName: Ref<string>
    currentForm: Ref<any>
    tags: Ref<string[]>
    contentWrapperRef: Ref<HTMLElement | null>
    lastSubmit: Ref<string>
    currentTemplate: ComputedRef<any>
    loginUsers: ComputedRef<any[]>
    seasonViewRef: Ref<{ refreshSeasons?: () => Promise<void> } | null>
    getCurrentAutoSubmitting: ComputedRef<boolean>
    saveTemplateSelection: (userUid: number, templateName: string) => void
    clearSavedSelection: () => void
}

/**
 * 模板管理：模板选择/加载/重置/保存、远程模板同步、模板命令、
 * 模板名编辑以及分区（category）选择。
 */
export const useTemplateManager = (context: TemplateManagerContext) => {
    const {
        templateLoading,
        selectedUser,
        currentTemplateName,
        currentForm,
        tags,
        contentWrapperRef,
        lastSubmit,
        currentTemplate,
        loginUsers,
        seasonViewRef,
        getCurrentAutoSubmitting,
        saveTemplateSelection,
        clearSavedSelection
    } = context

    const userConfigStore = useUserConfigStore()
    const utilsStore = useUtilsStore()

    // 分区数据
    const typeList = computed(() => utilsStore.typelist)
    const typeListV2 = computed(() => utilsStore.typeListV2)
    const selectedCategory = ref<any>(null)
    const selectedSubCategory = ref<any>(null)
    const categoryPopoverVisible = ref(false)

    // 模板名编辑相关
    const isEditingTemplateName = ref(false)
    const editingTemplateName = ref('')
    const templateNameInputRef = ref()

    // 监听表单分区变化，更新分区选择（双向绑定）
    watch(
        () => currentForm.value?.tid,
        (newTid: number | undefined) => {
            if (newTid && newTid > 0) {
                // 根据tid设置选中的分区
                setSelectedCategoryByTid(newTid)
            } else {
                // 如果没有分区信息，清空分区选择
                selectedCategory.value = null
                selectedSubCategory.value = null
            }
        }
    )

    // 处理模板名编辑点击 - 在模板加载时禁用
    const handleTemplateNameEdit = () => {
        if (!templateLoading.value) {
            startEditTemplateName()
        }
    }

    const refreshSeasonList = async () => {
        if (templateLoading.value) {
            return
        }

        if (!selectedUser.value?.uid) {
            utilsStore.showMessage('请先选择用户和模板', 'warning')
            return
        }

        try {
            await seasonViewRef.value?.refreshSeasons?.()
            utilsStore.showMessage('合集列表已刷新', 'success')
        } catch (error) {
            console.error('刷新合集列表失败:', error)
            utilsStore.showMessage(`刷新合集列表失败: ${error}`, 'error')
        }
    }

    // 选择模板
    const selectTemplate = async (user: any, templateName: string) => {
        // 如果正在加载模板，禁止切换
        if (templateLoading.value) {
            return
        }

        if (selectedUser.value === user && currentTemplateName.value === templateName) {
            // 如果已经选择了相同的用户和模板，则不需要切换
            return
        }

        templateLoading.value = true
        try {
            lastSubmit.value = ''

            // 切换模板时，按模板所属用户重新初始化投稿前信息与活动列表
            await utilsStore.initArchievePre(user.uid)
            await utilsStore.initTopicList(user.uid)

            selectedUser.value = user
            currentTemplateName.value = templateName

            // 滚动到顶部
            nextTick(() => {
                if (contentWrapperRef.value) {
                    contentWrapperRef.value.scrollTop = 0
                }
            })

            // 加载模板数据到表单
            await loadTemplate()

            // 保存模板选择到localStorage
            saveTemplateSelection(user.uid, templateName)

            // 如果模板有 aid，主动刷新模板数据
            const aid = currentTemplate.value?.aid
            setTimeout(async () => {
                if (aid) {
                    try {
                        if (
                            selectedUser.value?.uid === user.uid &&
                            currentTemplateName.value === templateName
                        ) {
                            const newTemplate = await getNewTemplateFromAv(user.uid, aid)
                            const currentTemplateData =
                                userConfigStore.configRoot?.config[user.uid].templates[templateName]
                            if (
                                currentTemplateData &&
                                hasUnsavedChanges(newTemplate, currentTemplateData)
                            ) {
                                await ElMessageBox.confirm(
                                    `检测到本地模板内容与bilibili不一致，是否刷新？（此操作会丢失所有未保存的更改）`,
                                    '',
                                    {
                                        confirmButtonText: '刷新并继续',
                                        cancelButtonText: '不刷新，仅显示当前',
                                        type: 'info'
                                    }
                                )
                                await reloadTemplateFromAV(user.uid, aid)
                            }
                        }
                    } catch (error) {
                        console.error('自动刷新模板数据失败:', error)
                    }
                }
            }, 666)
            console.log(`已切换到模板: ${user.username} - ${templateName}`)
        } catch (error) {
            console.error('切换模板失败:', error)
            utilsStore.showMessage(`切换模板失败: ${error}`, 'error')
        } finally {
            templateLoading.value = false
        }
    }

    const resetTemplate = async () => {
        if (!selectedUser.value || !currentTemplateName.value) {
            utilsStore.showMessage('请先选择用户和模板', 'warning')
            return
        }

        // 如果正在加载模板，禁止重置
        if (templateLoading.value) {
            utilsStore.showMessage('模板正在加载中，请稍后再试', 'warning')
            return
        }

        // 确认重置
        try {
            await ElMessageBox.confirm('确定要清除所有未保存的更改吗?', '', {
                confirmButtonText: '确定',
                cancelButtonText: '取消',
                type: 'warning'
            })

            templateLoading.value = true
            try {
                currentForm.value =
                    JSON.parse(
                        JSON.stringify(
                            userConfigStore.configBase?.config[selectedUser.value.uid]?.templates[
                                currentTemplateName.value
                            ]
                        )
                    ) || userConfigStore.createDefaultTemplate()
                utilsStore.showMessage('模板已重置', 'success')
            } finally {
                templateLoading.value = false
            }
        } catch (error) {
            // 用户取消了重置
            console.log('重置操作已取消')
        }
    }

    const getNewTemplateFromAv = async (userUid: number, aid: number) => {
        try {
            const newTemplate = (await utilsStore.getVideoDetail(userUid, aid.toString())) as any

            // 处理视频列表
            if (newTemplate.videos && Array.isArray(newTemplate.videos)) {
                for (const video of newTemplate.videos) {
                    video.id = video.filename
                    video.path = ''
                }
            }

            if (newTemplate.aid && (await utilsStore.getSeasonList(userUid))) {
                const season_id = await utilsStore.getVideoSeason(userUid, newTemplate.aid)

                if (season_id !== 0) {
                    const section_id = await utilsStore.seasonlist.find(
                        item => item.season_id === season_id
                    )?.section_id
                    newTemplate.season_id = season_id
                    newTemplate.section_id = section_id
                }
            }

            newTemplate.watermark = currentForm.value?.watermark
            return newTemplate
        } catch (error) {
            console.error('获取新模板失败: ', error)
            throw error
        }
    }

    const reloadTemplateFromAV = async (userUid: number, aid: number) => {
        // 如果正在加载模板，禁止重新加载
        if (templateLoading.value) {
            return
        }

        if (!selectedUser.value || selectedUser.value.uid !== userUid) {
            return
        }

        if (!currentForm.value || currentForm.value.aid !== aid) {
            return
        }

        templateLoading.value = true
        try {
            const newTemplate = await getNewTemplateFromAv(userUid, aid)
            currentForm.value = newTemplate
            utilsStore.showMessage('模板数据已刷新', 'success')
        } catch (error) {
            console.error('刷新失败: ', error)
            utilsStore.showMessage(`刷新失败: ${error}`, 'error')
            throw error
        } finally {
            templateLoading.value = false
        }
    }

    // 加载模板数据到表单
    const loadTemplate = async () => {
        try {
            // 如果没有模板，则使用默认模板配置
            if (!currentTemplate.value) {
                const defaultTemplate = userConfigStore.createDefaultTemplate()
                // 直接设置到配置中
                if (
                    selectedUser.value &&
                    currentTemplateName.value &&
                    userConfigStore.configRoot?.config
                ) {
                    const userConfig = userConfigStore.configRoot.config[selectedUser.value.uid]
                    if (userConfig) {
                        userConfig.templates[currentTemplateName.value] = defaultTemplate
                    }
                }

                // 清空标签
                tags.value = []

                // 清空分区选择
                selectedCategory.value = null
                selectedSubCategory.value = null

                // 等待所有更新完成
                await nextTick()

                return
            }

            const template = currentTemplate.value

            // 解析标签
            tags.value = template.tag
                ? template.tag.split(',').filter((tag: string) => tag.trim())
                : []

            // 设置选中的分区
            if (template.tid) {
                setSelectedCategoryByTid(template.tid)
            } else {
                // 如果没有分区信息，清空分区选择
                selectedCategory.value = null
                selectedSubCategory.value = null
            }

            // 等待所有更新完成
            await nextTick()

            // 模板数据已直接操作，无需保存基础状态
        } catch (error) {
            console.error('加载模板失败:', error)
            utilsStore.showMessage(`加载模板失败: ${error}`, 'error')
        }
    }

    // 处理模板命令
    const handleTemplateCommand = async (command: string, user: any, template: any) => {
        switch (command) {
            case 'duplicate':
                try {
                    const newName = `${template.name}_副本`
                    await userConfigStore.duplicateUserTemplate(user.uid, template.name, newName)
                    utilsStore.showMessage('模板复制成功', 'success')
                } catch (error) {
                    console.error('复制模板失败: ', error)
                    utilsStore.showMessage(`'复制模板失败: ${error}'`, 'error')
                }
                break

            case 'rename':
                try {
                    const { value: newName } = await ElMessageBox.prompt(
                        '请输入新的模板名称',
                        '重命名模板',
                        {
                            confirmButtonText: '确定',
                            cancelButtonText: '取消',
                            inputPlaceholder: '请输入模板名称',
                            inputValue: template.name,
                            inputValidator: (value: string) => {
                                if (!value || !value.trim()) {
                                    return '模板名称不能为空'
                                }
                                if (value.trim() === template.name) {
                                    return '新名称不能与原名称相同'
                                }
                                return true
                            }
                        }
                    )

                    const trimmedName = newName.trim()

                    // 检查是否已存在同名模板
                    const existingTemplate = userConfigStore.getUserTemplate(user.uid, trimmedName)
                    if (existingTemplate) {
                        utilsStore.showMessage('该名称的模板已存在，请使用其他名称', 'error')
                        return
                    }

                    await userConfigStore.renameUserTemplate(user.uid, template.name, trimmedName)

                    // 更新当前选择
                    if (
                        selectedUser.value?.uid === user.uid &&
                        currentTemplateName.value === template.name
                    ) {
                        currentTemplateName.value = trimmedName
                        // 更新localStorage中的模板选择记录
                        saveTemplateSelection(user.uid, trimmedName)
                    }

                    utilsStore.showMessage('模板重命名成功', 'success')
                } catch (error) {
                    if (error !== 'cancel') {
                        console.error('重命名模板失败: ', error)
                        utilsStore.showMessage(`'重命名模板失败: ${error}'`, 'error')
                    }
                }
                break

            case 'delete':
                try {
                    const template_name = template.name || currentTemplateName.value
                    await ElMessageBox.confirm(`确定要删除模板"${template_name}"吗？`, '确认删除', {
                        confirmButtonText: '确定',
                        cancelButtonText: '取消',
                        type: 'warning'
                    })

                    await userConfigStore.removeUserTemplate(user.uid, template_name)

                    // 如果删除的是当前选中的模板，清空选择
                    if (
                        selectedUser.value?.uid === user.uid &&
                        currentTemplateName.value === template_name
                    ) {
                        currentTemplateName.value = ''
                        selectedUser.value = null
                        // 清除localStorage中的模板选择记录
                        clearSavedSelection()
                    }

                    utilsStore.showMessage('模板删除成功', 'success')
                } catch (error) {
                    if (error !== 'cancel') {
                        console.error('删除模板失败: ', error)
                        utilsStore.showMessage(`'删除模板失败: ${error}'`, 'error')
                    }
                }
                break
        }
    }

    // 处理模板创建成功事件
    const handleTemplateCreated = async (userUid: number, templateName: string) => {
        if (getCurrentAutoSubmitting.value) {
            return
        }

        // 自动选择新创建的模板
        const targetUser = loginUsers.value.find(user => user.uid === userUid)
        if (targetUser) {
            selectedUser.value = targetUser
            currentTemplateName.value = templateName

            // 滚动到顶部
            nextTick(() => {
                if (contentWrapperRef.value) {
                    contentWrapperRef.value.scrollTop = 0
                }
            })

            templateLoading.value = true
            await loadTemplate()
            templateLoading.value = false

            // 保存新创建的模板选择
            saveTemplateSelection(userUid, templateName)
        }
    }

    // 保存模板
    const saveTemplate = async () => {
        if (!selectedUser.value || !currentTemplateName.value || !currentTemplate.value) {
            utilsStore.showMessage('请先选择模板', 'error')
            return
        }

        try {
            // 直接保存当前模板配置
            await userConfigStore.updateUserTemplate(
                selectedUser.value.uid,
                currentTemplateName.value,
                currentTemplate.value
            )

            // 模板数据已直接操作并保存，无需额外状态管理
        } catch (error) {
            console.error('保存模板失败: ', error)
            utilsStore.showMessage(`'保存模板失败: ${error}'`, 'error')
        }
    }

    // 分区选择相关
    const onCategoryChange = (categoryId: number) => {
        const category = typeList.value.find((item: any) => item.id === categoryId)
        selectedCategory.value = category
        selectedSubCategory.value = null
        if (currentForm.value) {
            currentForm.value.tid = 0
            // 上一行会触发watch事件，导致selectedCategory被清空
            nextTick(() => {
                selectedCategory.value = category
            })
        }
    }

    const onSubCategoryChange = (subCategoryId: number) => {
        if (selectedCategory.value && selectedCategory.value.children) {
            const subCategory = selectedCategory.value.children.find(
                (item: any) => item.id === subCategoryId
            )
            selectedSubCategory.value = subCategory
            if (currentForm.value) {
                currentForm.value.tid = subCategoryId
            }
            // 选择子分区后关闭popover
            categoryPopoverVisible.value = false
        }
    }

    // 根据tid设置选中的分区
    const setSelectedCategoryByTid = (tid: number) => {
        for (const category of typeList.value) {
            if (category.children) {
                const subCategory = category.children.find((item: any) => item.id === tid)
                if (subCategory) {
                    selectedCategory.value = category
                    selectedSubCategory.value = subCategory
                    return
                }
            }
        }
    }

    // 模板名编辑相关函数
    const startEditTemplateName = () => {
        isEditingTemplateName.value = true
        editingTemplateName.value = currentTemplateName.value
        nextTick(() => {
            templateNameInputRef.value?.focus()
        })
    }

    const saveTemplateName = async () => {
        const newName = editingTemplateName.value.trim()

        if (!newName) {
            utilsStore.showMessage('模板名称不能为空', 'error')
            cancelEditTemplateName()
            return
        }

        if (newName === currentTemplateName.value) {
            cancelEditTemplateName()
            return
        }

        if (!selectedUser.value) {
            utilsStore.showMessage('未选择用户', 'error')
            cancelEditTemplateName()
            return
        }

        try {
            const existingTemplate = userConfigStore.getUserTemplate(
                selectedUser.value.uid,
                newName
            )
            if (existingTemplate) {
                utilsStore.showMessage('该名称的模板已存在，请使用其他名称', 'error')
                return
            }

            await userConfigStore.renameUserTemplate(
                selectedUser.value.uid,
                currentTemplateName.value,
                newName
            )

            // 更新当前选择
            currentTemplateName.value = newName
            saveTemplateSelection(selectedUser.value.uid, newName)

            utilsStore.showMessage('模板重命名成功', 'success')
            isEditingTemplateName.value = false
        } catch (error) {
            console.error('重命名模板失败: ', error)
            utilsStore.showMessage(`重命名模板失败: ${error}`, 'error')
            cancelEditTemplateName()
        }
    }

    const cancelEditTemplateName = () => {
        isEditingTemplateName.value = false
        editingTemplateName.value = ''
    }

    return {
        typeList,
        typeListV2,
        selectedCategory,
        selectedSubCategory,
        categoryPopoverVisible,
        isEditingTemplateName,
        editingTemplateName,
        templateNameInputRef,
        handleTemplateNameEdit,
        refreshSeasonList,
        selectTemplate,
        resetTemplate,
        getNewTemplateFromAv,
        reloadTemplateFromAV,
        loadTemplate,
        handleTemplateCommand,
        handleTemplateCreated,
        saveTemplate,
        onCategoryChange,
        onSubCategoryChange,
        setSelectedCategoryByTid,
        startEditTemplateName,
        saveTemplateName,
        cancelEditTemplateName
    }
}
