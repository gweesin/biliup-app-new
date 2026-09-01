<template>
    <div class="main-view">
        <!-- 拖拽覆盖层 -->
        <div v-if="isDragOver" class="drag-overlay">
            <div class="drag-content">
                <el-icon class="drag-icon"><upload-filled /></el-icon>
                <h3>拖拽视频文件到此处</h3>
                <p>支持 MP4、AVI、MOV、MKV、WMV、FLV、M4V、WEBM 格式</p>
                <p v-if="!selectedUser || !currentTemplateName" class="warning-text">
                    请先选择用户和模板
                </p>
            </div>
        </div>

        <!-- 顶部导航栏 -->
        <el-header class="header">
            <div class="header-content">
                <div class="header-left">
                    <h2 class="app-title">Biliup APP</h2>
                    <div class="app-version">(v{{ currentVer }})</div>
                </div>
                <div class="header-center">
                    <el-button type="info" size="small" @click="exportLogs" title="导出日志">
                        导出日志
                    </el-button>
                    <el-button type="primary" size="small" @click="checkUpdate" title="检查更新">
                        检查更新
                    </el-button>
                    <el-button
                        type="success"
                        size="small"
                        @click="submitStatsDialogVisible = true"
                        title="提交统计"
                    >
                        提交统计
                    </el-button>
                </div>
                <div class="header-right">
                    <!-- 上传队列下拉框 -->
                    <UploadQueue />

                    <!-- 全局设置按钮 -->
                    <el-button
                        type="info"
                        size="small"
                        circle
                        @click="showGlobalConfigDialog = true"
                        title="全局设置"
                        class="global-config-btn"
                    >
                        <el-icon><setting /></el-icon>
                    </el-button>

                    <!-- 用户列表下拉框 -->
                    <UserList
                        @show-login="showLoginDialog = true"
                        @user-logout="handleLogoutUser"
                    />
                </div>
            </div>
        </el-header>

        <el-container class="main-container">
            <TemplateSidebar
                :user-templates="userTemplates"
                :template-loading="templateLoading"
                :auto-submitting-record="autoSubmittingRecord"
                :selected-user-uid="selectedUser?.uid ?? null"
                :current-template-name="currentTemplateName"
                :login-user-count="loginUsers.length"
                :has-unsaved-changes="checkTemplateHasUnsavedChanges"
                @show-login="showLoginDialog = true"
                @show-new-template="showNewTemplateDialog = true"
                @open-user-config="openUserConfig"
                @select-template="selectTemplate"
                @template-command="handleTemplateCommand"
            />

            <!-- 主要内容区域 -->
            <el-main class="main-content" v-if="currentForm">
                <div class="content-wrapper" ref="contentWrapperRef">
                    <div v-if="!selectedUser" class="no-selection">
                        <el-empty description="请选择用户和模板开始使用" />
                    </div>

                    <div v-else-if="!currentTemplateName" class="no-template">
                        <el-empty description="请选择模板或创建新模板">
                            <el-button type="primary" @click="showNewTemplateDialog = true">
                                新建模板
                            </el-button>
                        </el-empty>
                    </div>

                    <div v-else class="upload-form-container">
                        <div class="form-header">
                            <div class="template-name-container">
                                <h3 class="edit-bv-template-disaplay" v-if="currentTemplate?.aid">
                                    编辑稿件：
                                </h3>
                                <el-tooltip
                                    v-if="currentTemplate?.aid"
                                    content="刷新稿件数据"
                                    placement="top"
                                >
                                    <el-icon
                                        class="refresh-btn"
                                        @click.stop="
                                            reloadTemplateFromAV(
                                                selectedUser.uid,
                                                currentTemplate.aid
                                            )
                                        "
                                    >
                                        <refresh />
                                    </el-icon>
                                </el-tooltip>
                                <span
                                    v-if="currentTemplate?.aid && archiveStateDesc"
                                    class="archive-state-badge"
                                    :class="archiveStateClass"
                                >
                                    {{ archiveStateDesc }} ({{ archiveState }})
                                </span>
                                <h3
                                    v-if="!isEditingTemplateName"
                                    @click="handleTemplateNameEdit"
                                    class="template-name-display"
                                    :class="{ disabled: templateLoading }"
                                    :title="
                                        templateLoading
                                            ? '模板加载中，无法编辑'
                                            : '点击编辑模板名称'
                                    "
                                >
                                    {{ currentTemplateName }}
                                    <el-icon class="edit-hint-icon"><edit /></el-icon>
                                </h3>
                                <el-input
                                    v-else
                                    ref="templateNameInputRef"
                                    v-model="editingTemplateName"
                                    @blur="saveTemplateName"
                                    @keyup.enter="saveTemplateName"
                                    @keyup.esc="cancelEditTemplateName"
                                    class="template-name-input"
                                    size="large"
                                    :disabled="templateLoading"
                                />
                                <LastPublishedBadge
                                    ref="lastPublishedBadgeRef"
                                    :uid="selectedUser?.uid ?? 0"
                                    :title="currentTemplateName"
                                    @update:last-publish-time="lastPublishedTimeSec = $event"
                                />
                            </div>
                            <div class="header-actions">
                                <el-button @click="resetTemplate" :disabled="templateLoading"
                                    >放弃更改</el-button
                                >
                                <el-button
                                    type="primary"
                                    @click="saveTemplate"
                                    :disabled="templateLoading"
                                    >保存</el-button
                                >
                                <el-button
                                    @click="
                                        handleTemplateCommand('delete', selectedUser, {
                                            name: currentTemplateName,
                                            config: currentTemplate
                                        })
                                    "
                                    @click.stop
                                    trigger="click"
                                    type="danger"
                                    :disabled="templateLoading"
                                    >删除</el-button
                                >
                            </div>
                        </div>

                        <el-form :model="currentForm" label-width="80px" class="upload-form">
                            <!-- 基本信息 -->
                            <el-card
                                class="form-section"
                                :class="{ collapsed: cardCollapsed.basic }"
                            >
                                <template #header>
                                    <div class="card-header" @click="toggleCardCollapsed('basic')">
                                        <span>基本信息</span>
                                        <div class="header-actions">
                                            <el-button
                                                type="danger"
                                                text
                                                size="small"
                                                @click.stop="clearCardContent('basic')"
                                                title="清空基本信息"
                                                :disabled="templateLoading"
                                            >
                                                <el-icon><delete /></el-icon>
                                            </el-button>
                                            <el-icon
                                                class="collapse-icon"
                                                :class="{ collapsed: cardCollapsed.basic }"
                                            >
                                                <arrow-down />
                                            </el-icon>
                                        </div>
                                    </div>
                                </template>

                                <el-collapse-transition>
                                    <div v-show="!cardCollapsed.basic" class="card-content">
                                        <el-form-item label="视频标题" required>
                                            <el-input
                                                v-model="currentForm.title"
                                                placeholder="请输入视频标题"
                                                maxlength="80"
                                                show-word-limit
                                                :disabled="templateLoading"
                                            />
                                        </el-form-item>

                                        <el-form-item label="封面">
                                            <CoverUploader
                                                v-model="currentForm.cover"
                                                :title="currentForm.title"
                                                :uid="selectedUser.uid"
                                                :disabled="templateLoading"
                                            />
                                        </el-form-item>

                                        <el-form-item label="视频分区">
                                            <el-popover
                                                v-model:visible="categoryPopoverVisible"
                                                placement="bottom-start"
                                                :width="600"
                                                trigger="click"
                                                popper-class="category-popover"
                                            >
                                                <template #reference>
                                                    <el-button
                                                        class="category-trigger"
                                                        :type="
                                                            currentForm.tid ? 'primary' : 'default'
                                                        "
                                                        :disabled="
                                                            templateLoading ||
                                                            Boolean(currentForm.aid)
                                                        "
                                                    >
                                                        <span class="category-text">
                                                            <span v-if="selectedSubCategory">
                                                                {{ selectedCategory?.name }} >
                                                                {{ selectedSubCategory?.name }}
                                                            </span>
                                                            <span v-else class="placeholder"
                                                                >请选择分区</span
                                                            >
                                                        </span>
                                                        <el-icon class="arrow-icon">
                                                            <arrow-down />
                                                        </el-icon>
                                                    </el-button>
                                                </template>

                                                <div class="category-selector-panel">
                                                    <!-- 左侧主分区列表 -->
                                                    <div class="category-list">
                                                        <div
                                                            v-for="category in typeList"
                                                            :key="category.id"
                                                            class="category-item"
                                                            :class="{
                                                                active:
                                                                    selectedCategory?.id ===
                                                                    category.id
                                                            }"
                                                            @click="onCategoryChange(category.id)"
                                                        >
                                                            <span class="category-name">{{
                                                                category.name
                                                            }}</span>
                                                            <el-icon class="arrow-right">
                                                                <arrow-down
                                                                    style="
                                                                        transform: rotate(-90deg);
                                                                    "
                                                                />
                                                            </el-icon>
                                                        </div>
                                                    </div>

                                                    <!-- 右侧子分区列表 -->
                                                    <div class="subcategory-list">
                                                        <div
                                                            v-if="
                                                                selectedCategory &&
                                                                selectedCategory.children
                                                            "
                                                        >
                                                            <div
                                                                v-for="subCategory in selectedCategory.children"
                                                                :key="subCategory.id"
                                                                class="subcategory-item"
                                                                :class="{
                                                                    active:
                                                                        selectedSubCategory?.id ===
                                                                        subCategory.id
                                                                }"
                                                                @click="
                                                                    onSubCategoryChange(
                                                                        subCategory.id
                                                                    )
                                                                "
                                                                :title="
                                                                    subCategory.intro_original ||
                                                                    subCategory.desc
                                                                "
                                                            >
                                                                <div class="subcategory-content">
                                                                    <div class="subcategory-name">
                                                                        {{ subCategory.name }}
                                                                    </div>
                                                                    <div class="subcategory-desc">
                                                                        {{
                                                                            subCategory.desc !== ''
                                                                                ? subCategory.desc
                                                                                : subCategory.intro_original
                                                                        }}
                                                                    </div>
                                                                </div>
                                                            </div>
                                                        </div>
                                                        <div v-else class="empty-subcategory">
                                                            <el-empty
                                                                description="请选择左侧主分区"
                                                                :image-size="60"
                                                            />
                                                        </div>
                                                    </div>
                                                </div>
                                            </el-popover>
                                        </el-form-item>

                                        <el-form-item label="新版分区">
                                            <el-select
                                                v-model="tidV2SelectValue"
                                                placeholder="请选择新版分区"
                                                clearable
                                                filterable
                                                :disabled="
                                                    templateLoading || Boolean(currentForm.aid)
                                                "
                                            >
                                                <el-option
                                                    v-for="item in typeListV2"
                                                    :key="item.id"
                                                    :label="item.name"
                                                    :value="item.id"
                                                />
                                            </el-select>
                                        </el-form-item>

                                        <el-form-item label="版权声明">
                                            <el-radio-group
                                                v-model="currentForm.copyright"
                                                :disabled="templateLoading"
                                            >
                                                <el-radio :value="1">自制</el-radio>
                                                <el-radio :value="2">转载</el-radio>
                                            </el-radio-group>
                                        </el-form-item>

                                        <el-form-item
                                            label="转载来源"
                                            v-if="currentForm.copyright === 2"
                                        >
                                            <el-input
                                                v-model="currentForm.source"
                                                placeholder="请填写转载来源"
                                                :disabled="templateLoading"
                                            />
                                        </el-form-item>
                                    </div>
                                </el-collapse-transition>
                            </el-card>

                            <!-- 视频文件 -->
                            <el-card
                                class="form-section"
                                :class="{
                                    'drag-target': isDragOver,
                                    collapsed: cardCollapsed.videos
                                }"
                            >
                                <template #header>
                                    <div class="card-header">
                                        <div
                                            style="
                                                display: flex;
                                                align-items: center;
                                                gap: 12px;
                                                flex: 1;
                                            "
                                            @click="toggleCardCollapsed('videos')"
                                        >
                                            <span style="cursor: pointer">视频文件</span>
                                            <el-button
                                                type="success"
                                                size="small"
                                                @click.stop="checkVideoStatus"
                                                v-if="
                                                    currentForm.videos &&
                                                    currentForm.videos.length > 0 &&
                                                    currentTemplate?.aid
                                                "
                                                :disabled="templateLoading"
                                            >
                                                视频转码状态
                                            </el-button>
                                        </div>
                                        <div class="header-actions">
                                            <span v-if="isDragOver" class="drag-hint"
                                                >拖拽文件到此处添加</span
                                            >
                                            <el-icon
                                                class="collapse-icon"
                                                :class="{ collapsed: cardCollapsed.videos }"
                                                @click="toggleCardCollapsed('videos')"
                                            >
                                                <arrow-down />
                                            </el-icon>
                                        </div>
                                    </div>
                                </template>

                                <el-collapse-transition>
                                    <div v-show="!cardCollapsed.videos" class="card-content">
                                        <VideoList
                                            v-model:videos="videos"
                                            :is-drag-over="isDragOver"
                                            :uploading="uploading"
                                            :template-title="currentTemplateName"
                                            :disabled="templateLoading"
                                            :uid="selectedUser.uid"
                                            :last-publish-time="lastPublishedTimeSec"
                                            @select-video="selectVideoWithTauri"
                                            @clear-all-videos="clearAllVideos"
                                            @remove-file="removeUploadedFile"
                                            @create-upload="createUpload"
                                            @add-videos-to-form="handleAddVideosToForm"
                                            @submit-template="handleSubmitTemplate"
                                        />
                                    </div>
                                </el-collapse-transition>
                            </el-card>

                            <!-- 标签设置 -->
                            <el-card
                                class="form-section"
                                :class="{ collapsed: cardCollapsed.tags }"
                            >
                                <template #header>
                                    <div class="card-header" @click="toggleCardCollapsed('tags')">
                                        <span>标签设置</span>
                                        <div class="header-actions">
                                            <el-button
                                                type="danger"
                                                text
                                                size="small"
                                                @click.stop="clearCardContent('tags')"
                                                title="清空标签设置"
                                                :disabled="templateLoading"
                                            >
                                                <el-icon><delete /></el-icon>
                                            </el-button>
                                            <el-icon
                                                class="collapse-icon"
                                                :class="{ collapsed: cardCollapsed.tags }"
                                            >
                                                <arrow-down />
                                            </el-icon>
                                        </div>
                                    </div>
                                </template>

                                <el-collapse-transition>
                                    <div v-show="!cardCollapsed.tags" class="card-content">
                                        <el-form-item label="视频标签">
                                            <TagView
                                                ref="tagViewRef"
                                                v-model="tags"
                                                :locked-first-tag="lockedFirstTag"
                                                :disabled="templateLoading"
                                            />
                                        </el-form-item>

                                        <el-form-item label="参与活动">
                                            <TopicView
                                                v-model="currentForm.mission_id"
                                                v-model:topic-id="currentForm.topic_id"
                                                v-model:topic-name="currentForm.topic_name"
                                                :user-uid="selectedUser?.uid"
                                                mode="selector"
                                                :disabled="
                                                    templateLoading || Boolean(currentForm.aid)
                                                "
                                            />
                                        </el-form-item>

                                        <el-form-item v-if="!staffFieldDisabled" label="联合投稿">
                                            <StaffView
                                                v-model="currentForm.staff"
                                                :user-uid="selectedUser?.uid"
                                                :is-edit-mode="Boolean(currentForm.aid)"
                                                :role-options="availableStaffRoles"
                                                :max-staff="maxStaffCount"
                                                :disabled="templateLoading"
                                            />
                                        </el-form-item>
                                    </div>
                                </el-collapse-transition>
                            </el-card>

                            <!-- 视频描述 -->
                            <el-card
                                class="form-section"
                                :class="{ collapsed: cardCollapsed.description }"
                            >
                                <template #header>
                                    <div
                                        class="card-header"
                                        @click="toggleCardCollapsed('description')"
                                    >
                                        <span>视频描述</span>
                                        <div class="header-actions">
                                            <el-button
                                                type="danger"
                                                text
                                                size="small"
                                                @click.stop="clearCardContent('description')"
                                                title="清空视频描述"
                                                :disabled="templateLoading"
                                            >
                                                <el-icon><delete /></el-icon>
                                            </el-button>
                                            <el-icon
                                                class="collapse-icon"
                                                :class="{ collapsed: cardCollapsed.description }"
                                            >
                                                <arrow-down />
                                            </el-icon>
                                        </div>
                                    </div>
                                </template>

                                <el-collapse-transition>
                                    <div v-show="!cardCollapsed.description" class="card-content">
                                        <DescView
                                            v-model:desc="currentForm.desc"
                                            v-model:desc-v2="currentForm.desc_v2"
                                            v-model:dynamic="currentForm.dynamic"
                                            :user-uid="selectedUser?.uid"
                                            :disabled="templateLoading"
                                        />
                                    </div>
                                </el-collapse-transition>
                            </el-card>

                            <!-- 高级选项 -->
                            <el-card
                                class="form-section"
                                :class="{ collapsed: cardCollapsed.advanced }"
                            >
                                <template #header>
                                    <div
                                        class="card-header"
                                        @click="toggleCardCollapsed('advanced')"
                                    >
                                        <span>高级选项</span>
                                        <div class="header-actions">
                                            <el-button
                                                type="danger"
                                                text
                                                size="small"
                                                @click.stop="clearCardContent('advanced')"
                                                title="清空高级选项"
                                                :disabled="templateLoading"
                                            >
                                                <el-icon><delete /></el-icon>
                                            </el-button>
                                            <el-icon
                                                class="collapse-icon"
                                                :class="{ collapsed: cardCollapsed.advanced }"
                                            >
                                                <arrow-down />
                                            </el-icon>
                                        </div>
                                    </div>
                                </template>

                                <el-collapse-transition>
                                    <div v-show="!cardCollapsed.advanced" class="card-content">
                                        <el-form-item label="开启水印">
                                            <div div class="checkbox-group">
                                                <el-checkbox
                                                    v-model="currentForm.watermark"
                                                    :true-value="1"
                                                    :false-value="0"
                                                    :disabled="templateLoading"
                                                >
                                                    开启 (本功能只对本次上传的视频生效)
                                                </el-checkbox>
                                            </div>
                                        </el-form-item>
                                        <el-form-item v-if="!currentForm.aid" label="定时发布">
                                            <DatePicker
                                                v-model="dtimeDate"
                                                placeholder="选择发布时间"
                                                :disabled="templateLoading"
                                            />
                                        </el-form-item>

                                        <el-form-item label="字幕设置">
                                            <el-checkbox
                                                v-model="currentForm.open_subtitle"
                                                :disabled="templateLoading"
                                            >
                                                开启字幕功能
                                            </el-checkbox>
                                        </el-form-item>

                                        <el-form-item label="互动功能">
                                            <div class="interactive-setting-row">
                                                <el-checkbox
                                                    v-model="currentForm.interactive"
                                                    :true-value="1"
                                                    :false-value="0"
                                                    :disabled="templateLoading"
                                                >
                                                    开启
                                                </el-checkbox>

                                                <el-tooltip
                                                    content="勾选后本视频将被投稿为互动视频，需在规定时间内完成剧情树配置，否则系统可能回收稿件。"
                                                    placement="top"
                                                >
                                                    <el-icon
                                                        class="interactive-help-icon"
                                                        @click.stop="showInteractiveInfoDialog"
                                                    >
                                                        <QuestionFilled />
                                                    </el-icon>
                                                </el-tooltip>
                                            </div>
                                        </el-form-item>

                                        <el-form-item label="加入合集">
                                            <div class="season-refresh-row">
                                                <SeasonView
                                                    ref="seasonViewRef"
                                                    v-model="currentForm.season_id"
                                                    v-model:section-id="currentForm.section_id"
                                                    :user-uid="selectedUser?.uid"
                                                    :disabled="templateLoading"
                                                />
                                                <el-tooltip content="刷新合集列表" placement="top">
                                                    <el-button
                                                        type="info"
                                                        size="small"
                                                        circle
                                                        class="season-refresh-btn"
                                                        :disabled="
                                                            templateLoading || !selectedUser?.uid
                                                        "
                                                        @click="refreshSeasonList"
                                                    >
                                                        <el-icon><refresh /></el-icon>
                                                    </el-button>
                                                </el-tooltip>
                                            </div>
                                        </el-form-item>

                                        <el-form-item
                                            v-if="currentForm.season_id && !currentForm.aid"
                                            label=""
                                        >
                                            <el-checkbox
                                                v-model="currentForm.no_disturbance"
                                                :true-value="1"
                                                :false-value="0"
                                                :disabled="templateLoading"
                                            >
                                                此稿件不产生更新推送
                                            </el-checkbox>
                                        </el-form-item>

                                        <el-form-item label="音质设置">
                                            <div class="checkbox-group">
                                                <el-checkbox
                                                    v-model="currentForm.dolby"
                                                    :true-value="1"
                                                    :false-value="0"
                                                    :disabled="templateLoading"
                                                >
                                                    杜比音效
                                                </el-checkbox>
                                                <el-checkbox
                                                    v-model="currentForm.lossless_music"
                                                    :true-value="1"
                                                    :false-value="0"
                                                    :disabled="templateLoading"
                                                >
                                                    无损音乐
                                                </el-checkbox>
                                            </div>
                                        </el-form-item>

                                        <el-form-item label="内容设置">
                                            <div class="checkbox-group">
                                                <el-checkbox
                                                    v-model="currentForm.no_reprint"
                                                    :true-value="1"
                                                    :false-value="0"
                                                    :disabled="templateLoading"
                                                >
                                                    禁止转载
                                                </el-checkbox>
                                                <el-checkbox
                                                    v-model="currentForm.open_elec"
                                                    :true-value="1"
                                                    :false-value="0"
                                                    :disabled="templateLoading"
                                                >
                                                    开启充电
                                                </el-checkbox>
                                            </div>
                                        </el-form-item>

                                        <el-form-item label="互动管理">
                                            <div class="checkbox-group">
                                                <el-checkbox
                                                    v-model="currentForm.up_selection_reply"
                                                    :true-value="1"
                                                    :false-value="0"
                                                    :disabled="templateLoading"
                                                >
                                                    UP主精选评论
                                                </el-checkbox>
                                                <el-checkbox
                                                    v-model="currentForm.up_close_reply"
                                                    :true-value="1"
                                                    :false-value="0"
                                                    :disabled="templateLoading"
                                                >
                                                    关闭评论
                                                </el-checkbox>
                                                <el-checkbox
                                                    v-model="currentForm.up_close_danmu"
                                                    :true-value="1"
                                                    :false-value="0"
                                                    :disabled="templateLoading"
                                                >
                                                    关闭弹幕
                                                </el-checkbox>
                                            </div>
                                        </el-form-item>

                                        <el-form-item label="可见性">
                                            <el-checkbox
                                                v-model="currentForm.is_only_self"
                                                :true-value="1"
                                                :false-value="0"
                                                :disabled="templateLoading"
                                            >
                                                仅自己可见
                                            </el-checkbox>
                                        </el-form-item>
                                        <el-form-item
                                            v-if="Boolean(currentForm.aid)"
                                            label="全景视频"
                                        >
                                            <el-checkbox
                                                v-model="currentForm.is_360"
                                                :true-value="1"
                                                :false-value="-1"
                                                :disabled="templateLoading"
                                            >
                                            </el-checkbox>
                                        </el-form-item>
                                    </div>
                                </el-collapse-transition>
                            </el-card>

                            <!-- 上传操作区域 -->
                            <div class="upload-actions">
                                <el-button
                                    v-if="!separateSubmitting"
                                    type="primary"
                                    size="large"
                                    :loading="submitting"
                                    @click="submitTemplate"
                                    :disabled="
                                        separateSubmitting ||
                                        templateLoading ||
                                        !currentForm.videos ||
                                        currentForm.videos.length === 0 ||
                                        !currentForm.title ||
                                        currentForm.title.trim() === ''
                                    "
                                >
                                    <el-icon v-if="!allFilesUploaded && !submitting"
                                        ><loading
                                    /></el-icon>
                                    <el-icon v-else-if="!submitting"><check /></el-icon>
                                    {{
                                        !getCurrentAutoSubmitting
                                            ? currentTemplate?.aid
                                                ? '编辑稿件'
                                                : '新增单个稿件'
                                            : '上传完成后自动提交'
                                    }}
                                </el-button>
                                <div
                                    v-if="
                                        currentForm && !currentForm.aid && !getCurrentAutoSubmitting
                                    "
                                    class="multi-submit-entry"
                                >
                                    <el-button
                                        type="warning"
                                        size="large"
                                        :disabled="
                                            !separateSubmitting &&
                                            (submitting ||
                                                templateLoading ||
                                                !currentForm.videos ||
                                                currentForm.videos.length === 0)
                                        "
                                        @click="
                                            separateSubmitting
                                                ? stopSeparateSubmit()
                                                : submitTemplateAsSeparatePosts()
                                        "
                                    >
                                        {{
                                            separateSubmitting
                                                ? '停止多稿件提交'
                                                : '以多稿件模式分别提交视频'
                                        }}
                                    </el-button>
                                    <el-tooltip
                                        content="使用此功能时，将不再以分p模式提交视频，而是针对每一个视频单独提交一份稿件，稿件名即为当前的‘分p’名，其他内容完全复用当前模板内容，无法进行自定义。每条视频提交成功后会自动从当前视频列表移除。"
                                        placement="top"
                                    >
                                        <el-icon class="multi-submit-help-icon">
                                            <QuestionFilled />
                                        </el-icon>
                                    </el-tooltip>
                                </div>
                                <div class="form-tip" v-if="separateSubmitting">
                                    <div>上传完成数量: {{ separateSubmitUploadedCount }}</div>
                                    <div>提交完成数量: {{ separateSubmitCompletedCount }}</div>
                                    <div>总数量: {{ separateSubmitTotalCount }}</div>
                                </div>
                                <div class="form-tip" v-if="lastSubmit">
                                    <div>最后提交时间</div>
                                    <div>{{ lastSubmit }}</div>
                                </div>
                            </div>
                        </el-form>
                    </div>
                </div>
            </el-main>
        </el-container>

        <!-- 新建模板组件 -->
        <NewTemplete
            ref="newTemplateRef"
            v-model="showNewTemplateDialog"
            @template-created="handleTemplateCreated"
        />

        <!-- 登录对话框 -->
        <el-dialog
            v-model="showLoginDialog"
            width="500px"
            :show-close="false"
            :close-on-click-modal="true"
            :close-on-press-escape="false"
            :before-close="handleLoginDialogClose"
            class="login-dialog"
            top="5vh"
        >
            <div class="login-dialog-content" @click.stop>
                <LoginView
                    @login-success="handleLoginSuccess"
                    @loading-change="loginLoading = $event"
                />
            </div>
        </el-dialog>

        <!-- 用户配置对话框 -->
        <UserConfig v-model="userConfigVisible" :user="configUser" />

        <!-- 视频状态对话框 -->
        <VideoStatus
            v-model="showVideoStatusDialog"
            :videos="currentForm?.videos || []"
            :user="selectedUser"
            :template-aid="currentTemplate?.aid"
            @reload-template="
                () =>
                    selectedUser &&
                    currentTemplate?.aid &&
                    reloadTemplateFromAV(selectedUser.uid, currentTemplate.aid)
            "
        />

        <SubmitStatsPage
            v-model="submitStatsDialogVisible"
            :stats="submitStats"
            @clear="clearSubmitStats"
        />

        <!-- 全局配置对话框 -->
        <GlobalConfigView v-model="showGlobalConfigDialog" />
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, watch, onUnmounted } from 'vue'
import { useAuthStore } from '../stores/auth'
import { useUserConfigStore } from '../stores/user_config'
import { useUtilsStore } from '../stores/utils'
import { useUploadStore } from '../stores/upload'
import { ElMessageBox } from 'element-plus'
import {
    ArrowDown,
    UploadFilled,
    Check,
    Edit,
    Setting,
    Refresh,
    Delete,
    QuestionFilled
} from '@element-plus/icons-vue'
import { save } from '@tauri-apps/plugin-dialog'
import { copyFile, remove } from '@tauri-apps/plugin-fs'
import { openUrl } from '@tauri-apps/plugin-opener'
import { hasTemplateUnsavedChanges } from '../utils/templateDiff'
import { useCardCollapse } from '../composables/useCardCollapse'
import { useCardContent } from '../composables/useCardContent'
import { useTemplateSelection } from '../composables/useTemplateSelection'
import { useVideoImport } from '../composables/useVideoImport'
import { useTemplateManager } from '../composables/useTemplateManager'
import { useInteractiveDialogs } from '../composables/useInteractiveDialogs'
import { useUserActions } from '../composables/useUserActions'
import LoginView from '../components/LoginView.vue'
import TemplateSidebar from '../components/TemplateSidebar.vue'
import UserConfig from '../components/UserConfig.vue'
import TopicView from '../components/TopicView.vue'
import SeasonView from '../components/SeasonView.vue'
import UploadQueue from '../components/UploadQueue.vue'
import GlobalConfigView from '../components/GlobalConfig.vue'
import NewTemplete from '../components/NewTemplete.vue'
import VideoList from '../components/VideoList.vue'
import UserList from '../components/UserList.vue'
import VideoStatus from '../components/VideoStatus.vue'
import TagView from '../components/TagView.vue'
import DatePicker from '../components/DatePicker.vue'
import StaffView from '../components/StaffView.vue'
import DescView from '../components/DescView.vue'
import SubmitStatsPage from '../components/SubmitStatsPage.vue'
import CoverUploader from '../components/CoverUploader.vue'
import LastPublishedBadge from '../components/LastPublishedBadge.vue'
import type { SubmitStats, SubmitStatsInput } from '../types/submit'
import { useSeparateSubmit } from '../composables/useSeparateSubmit'

const authStore = useAuthStore()
const userConfigStore = useUserConfigStore()
const uploadStore = useUploadStore()
const utilsStore = useUtilsStore()

// 计算属性
const loginUsers = computed(() => authStore.loginUsers)
const userTemplates = computed(() => userConfigStore.userTemplates)

const currentVer = ref<string>('')

// 响应式数据
const selectedUser = ref<any>(null)
const currentTemplateName = ref<string>('')
const lastPublishedBadgeRef = ref<InstanceType<typeof LastPublishedBadge> | null>(null)
// LastPublishedBadge 计算出的上次发布时间（秒），传给 VideoList 作为排期起始基准
const lastPublishedTimeSec = ref(0)
const showNewTemplateDialog = ref(false)
const showLoginDialog = ref(false)
const showGlobalConfigDialog = ref(false)
const submitStatsDialogVisible = ref(false)
const loginLoading = ref(false)
const uploading = ref(false)
const submitting = ref(false)
const templateLoading = ref(false) // 模板加载状态锁

// 视频状态对话框
const showVideoStatusDialog = ref(false)

// 组件引用
const newTemplateRef = ref<InstanceType<typeof NewTemplete> | null>(null)
const tagViewRef = ref<InstanceType<typeof TagView> | null>(null)
const seasonViewRef = ref<InstanceType<typeof SeasonView> | null>(null)
const submitStats = ref<SubmitStats>({
    startedAt: new Date().toLocaleString(),
    totalCount: 0,
    successCount: 0,
    failCount: 0,
    records: []
})

const recordSubmitStats = (record: SubmitStatsInput) => {
    submitStats.value.totalCount += 1
    if (record.status === 'success') {
        submitStats.value.successCount += 1
    } else {
        submitStats.value.failCount += 1
    }

    submitStats.value.records.unshift({
        time: new Date().toLocaleString(),
        user: record.user,
        mode: record.mode,
        templateName: record.templateName,
        status: record.status,
        statusText: record.status === 'success' ? '成功' : '失败',
        bvid: record.bvid || '-',
        videoName: record.videoName || '-',
        error: record.error ? String(record.error) : '-'
    })
}

const clearSubmitStats = () => {
    submitStats.value.startedAt = new Date().toLocaleString()
    submitStats.value.totalCount = 0
    submitStats.value.successCount = 0
    submitStats.value.failCount = 0
    submitStats.value.records = []
    utilsStore.showMessage('提交统计内存已清空', 'success')
}

const {
    autoSubmittingRecord,
    separateSubmittingRecord,
    lastSubmit,
    separateSubmitting,
    separateSubmitUploadedCount,
    separateSubmitCompletedCount,
    separateSubmitTotalCount,
    getCurrentAutoSubmitting,
    hasAnyAutoSubmitting,
    hasAnySeparateSubmitting,
    setAutoSubmitting,
    resetAllSubmitStates,
    parseTemplateKey,
    checkAutoSubmitAll,
    performTemplateSubmit,
    stopSeparateSubmit,
    submitTemplateAsSeparatePosts
} = useSeparateSubmit({
    selectedUser,
    currentTemplateName,
    submitting,
    lastPublishedBadgeRef,
    newTemplateRef,
    loginUsers,
    recordSubmitStats,
    autoStartWaitingTasks: () => autoStartWaitingTasks(),
    reloadTemplateFromAV: (uid, aid) => reloadTemplateFromAV(uid, aid)
})

const { cardCollapsed, toggleCardCollapsed, restoreCardCollapsedState } = useCardCollapse()

// 内容容器引用
const contentWrapperRef = ref<HTMLElement | null>(null)

// 用户配置相关

let generalUpdateTimer: ReturnType<typeof setInterval> | null = null
// 轮询防重入标志，避免上一次请求未完成时重复发起
let queuePolling = false

// 判断上传队列中是否有需要轮询的活跃任务

const currentTemplate = computed(() => {
    if (!selectedUser.value || !currentTemplateName.value || !userConfigStore.configRoot?.config) {
        return null
    }
    const userConfig = userConfigStore.configRoot.config[selectedUser.value.uid]
    if (!userConfig || !userConfig.templates[currentTemplateName.value]) {
        return null
    }
    return userConfig.templates[currentTemplateName.value]
})

const archiveState = computed(() => {
    const value = currentTemplate.value?.state
    return typeof value === 'number' ? value : 0
})

const archiveStateDesc = computed(() => {
    const desc = currentTemplate.value?.state_desc || ''
    return desc.trim()
})

const archiveStateClass = computed(() => {
    if (archiveState.value === 0) {
        return 'state-ok'
    }
    if (archiveState.value > 0) {
        return 'state-neutral'
    }
    return 'state-error'
})

// 当前表单数据 - 直接操作模板配置
const currentForm = computed({
    get() {
        return currentTemplate.value
    },
    set(value) {
        if (
            !selectedUser.value ||
            !currentTemplateName.value ||
            !userConfigStore.configRoot?.config ||
            !value
        ) {
            return
        }
        const userConfig = userConfigStore.configRoot.config[selectedUser.value.uid]
        if (userConfig && userConfig.templates[currentTemplateName.value]) {
            userConfig.templates[currentTemplateName.value] = value
        }
    }
})

const tags = ref<string[]>([])

const lockedFirstTag = computed(() => {
    const first = tags.value[0] || ''
    const topicName = currentForm.value?.topic_name || ''
    if (!first || !topicName) {
        return undefined
    }
    return first === topicName ? first : undefined
})

const commonStaffConf = computed(() => utilsStore.common_staff_conf)
const availableStaffRoles = computed<string[]>(() => {
    const titles = commonStaffConf.value?.titles
    return Array.isArray(titles) ? (titles as string[]) : []
})
const maxStaffCount = computed<number>(() => commonStaffConf.value?.max_staff || 10)
const missionSupportsStaff = computed(() => {
    const missionId = Number(currentForm.value?.mission_id || 0)
    if (!missionId) {
        return false
    }
    return (commonStaffConf.value?.missions || []).includes(missionId)
})

const currentUserFans = computed<number | null>(() => {
    const archivePre = utilsStore.archieve_pre as any
    return archivePre?.myinfo?.follower
})

const lowFansNeedsSupportedMission = computed(() => {
    if (currentUserFans.value === null) {
        return false
    }
    return currentUserFans.value < 1000
})

const staffFieldDisabled = computed(() => {
    const onlySelfInvisible = Number(currentForm.value?.is_only_self || 0) === 1
    const blockedByFansRule = lowFansNeedsSupportedMission.value
    return onlySelfInvisible || (blockedByFansRule && !missionSupportsStaff.value)
})

// 日期选择器的计算属性 - 处理时间戳转换
const dtimeDate = computed({
    get() {
        return currentForm.value?.dtime ? new Date(currentForm.value.dtime * 1000) : null
    },
    set(value: Date | null) {
        if (currentForm.value) {
            currentForm.value.dtime = value ? Math.floor(value.getTime() / 1000) : undefined
        }
    }
})

const tidV2SelectValue = computed<number | undefined>({
    get() {
        const tidV2 = Number(currentForm.value?.tid_v2 || 0)
        return tidV2 > 0 ? tidV2 : undefined
    },
    set(value) {
        if (!currentForm.value) {
            return
        }

        const tidV2 = Number(value || 0)
        currentForm.value.tid_v2 = tidV2
    }
})

// 视频数组的计算属性 - 确保始终返回数组
const videos = computed({
    get() {
        return currentForm.value?.videos || []
    },
    set(value: any[]) {
        if (currentForm.value) {
            currentForm.value.videos = value
        }
    }
})

// 检查指定模板是否有未保存的改动
const checkTemplateHasUnsavedChanges = (uid: number, templateName: string): boolean => {
    return hasTemplateUnsavedChanges(
        userConfigStore.configRoot,
        userConfigStore.configBase,
        uid,
        templateName
    )
}

// 生命周期
// 监听标签变化，更新表单数据
watch(
    () => tags.value,
    (newTags: string[]) => {
        if (currentForm.value) {
            currentForm.value.tag = newTags.join(',')
        }
    },
    { deep: true }
)

// 监听表单标签变化，更新标签数组
watch(
    () => currentForm.value?.tag,
    (newTag: string | undefined) => {
        const newTags = newTag ? newTag.split(',').filter(tag => tag.trim()) : []
        if (JSON.stringify(newTags) !== JSON.stringify(tags.value)) {
            tags.value = newTags
        }
    }
)

watch(
    () => {
        const singleKeys = Object.keys(autoSubmittingRecord.value).sort()
        const separateKeys = Object.keys(separateSubmittingRecord.value).sort()

        const singleState = singleKeys
            .map(key => {
                const parsed = parseTemplateKey(key)
                if (!parsed) {
                    return `${key}:invalid`
                }

                const { uid, templateName } = parsed
                const template =
                    userConfigStore.configRoot?.config?.[uid]?.templates?.[templateName]
                if (!template?.videos) {
                    return `${key}:missing`
                }
                const state = template.videos
                    .map(video => `${video.id}:${video.complete ? 1 : 0}:${video.path || ''}`)
                    .join(',')
                return `${key}:${state}`
            })
            .join('|')

        const separateState = separateKeys
            .map(key => {
                const parsed = parseTemplateKey(key)
                if (!parsed) {
                    return `${key}:invalid`
                }

                const { uid, templateName } = parsed
                const template =
                    userConfigStore.configRoot?.config?.[uid]?.templates?.[templateName]
                if (!template?.videos) {
                    return `${key}:missing`
                }

                const state = template.videos
                    .map(video => `${video.id}:${video.complete ? 1 : 0}:${video.path || ''}`)
                    .join(',')
                return `${key}:${state}`
            })
            .join('|')

        return `single:${singleState}||separate:${separateState}`
    },
    () => {
        if (hasAnyAutoSubmitting.value || hasAnySeparateSubmitting.value) {
            void checkAutoSubmitAll()
        }
    },
    { immediate: true }
)

const { showInteractiveInfoDialog } = useInteractiveDialogs({
    currentForm,
    selectedUser,
    templateLoading
})

let keyboardCleanup: (() => void) | null = null

const forwardConsole = (fnName: keyof Console, logger: (level: string, ...args: any[]) => void) => {
    const original = console[fnName] as (...args: any[]) => void
    ;(console as any)[fnName] = (...args: any[]) => {
        original(...args)
        logger(fnName as string, ...args)
    }
}

onMounted(async () => {
    await initializeData()
    await setupDragAndDrop()
    keyboardCleanup = await setupKeyboardShortcuts()

    forwardConsole('log', utilsStore.log)
    forwardConsole('error', utilsStore.log)
    forwardConsole('warn', utilsStore.log)

    // 禁用右键菜单刷新
    document.addEventListener('contextmenu', (event: MouseEvent) => {
        event.preventDefault()
    })
})

// 在组件卸载时清理
onUnmounted(() => {
    if (keyboardCleanup) {
        keyboardCleanup()
    }

    if (generalUpdateTimer) {
        clearInterval(generalUpdateTimer)
        generalUpdateTimer = null
    }

    // 清理所有自动提交与多稿件提交状态
    resetAllSubmitStates()
})

// 初始化数据
const initializeData = async () => {
    try {
        currentVer.value = (await utilsStore.getCurrentVersion()) as string
        // 获取登录用户
        await authStore.getLoginUsers()

        // 构建用户模板列表
        if (loginUsers.value.length > 0) {
            await utilsStore.initArchievePre(loginUsers.value[0].uid)
            await utilsStore.initTopicList(loginUsers.value[0].uid)
            await userConfigStore.ensureUserTemplatesReady()
            await uploadStore.getUploadQueue()
            if (!generalUpdateTimer) {
                generalUpdateTimer = setInterval(async () => {
                    // 无登录用户时不轮询
                    if (authStore.loginUsers.length === 0) {
                        return
                    }
                    // 避免上一次请求未完成时重复发起
                    if (queuePolling) {
                        return
                    }
                    // 没有活跃上传任务时跳过，避免无效的后端请求
                    if (!hasActiveUploadTasks()) {
                        return
                    }
                    queuePolling = true
                    try {
                        await uploadStore.getUploadQueue()
                        syncCompletedTasksFromQueue()
                    } catch (error) {
                        console.error('轮询上传队列失败:', error)
                    } finally {
                        queuePolling = false
                    }
                }, 2000) // 上传进度轮询周期
            }
        }

        setTimeout(async () => {
            await restoreTemplateSelection()
            restoreCardCollapsedState()
        }, 100)
    } catch (error) {
        console.error('初始化数据失败: ', error)
        utilsStore.showMessage(`'初始化数据失败: ${error}'`, 'error')
    }
}

// 设置键盘快捷键
const setupKeyboardShortcuts = async () => {
    const handleKeydown = (event: KeyboardEvent) => {
        // 禁用 F5 刷新
        if (!event.ctrlKey && event.key === 'F5') {
            event.preventDefault()
            return
        }

        // Ctrl+F5 刷新页面
        if (event.ctrlKey && event.key === 'F5') {
            event.preventDefault()
            window.location.reload()
            return
        }

        if (event.ctrlKey && event.key === 'r') {
            event.preventDefault()
            if (selectedUser.value && currentTemplateName.value) {
                resetTemplate()
            }
            return
        }

        // Ctrl+S 保存模板
        if (event.ctrlKey && event.key === 's') {
            event.preventDefault()
            if (selectedUser.value && currentTemplateName.value) {
                saveTemplate()
            }
        }
    }

    document.addEventListener('keydown', handleKeydown)

    // 返回清理函数
    return () => {
        document.removeEventListener('keydown', handleKeydown)
    }
}

// 切换卡片折叠状态
const {
    saveTemplateSelection,
    restoreTemplateSelection,
    clearSavedSelection,
    clearSavedSelectionForUser
} = useTemplateSelection({
    loginUsers,
    userTemplates,
    selectTemplate: (user, templateName) => selectTemplate(user, templateName),
    toggleUserExpanded: uid => toggleUserExpanded(uid),
    showMessage: (message, type) => utilsStore.showMessage(message, type)
})

const {
    isDragOver,
    setupDragAndDrop,
    selectVideoWithTauri,
    clearAllVideos,
    removeUploadedFile,
    createUpload,
    handleAddVideosToForm
} = useVideoImport({
    currentForm,
    selectedUser,
    currentTemplateName,
    templateLoading,
    uploading,
    autoStartWaitingTasks: () => autoStartWaitingTasks()
})

// 处理登录成功
const handleLoginSuccess = async () => {
    showLoginDialog.value = false
    utilsStore.showMessage('登录成功', 'success')

    await userConfigStore.saveConfig()
    // 刷新所有数据
    await refreshAllData()
}

// 处理登录对话框关闭
const handleLoginDialogClose = async (done: () => void) => {
    if (loginLoading.value) {
        utilsStore.showMessage('登录过程中无法取消', 'warning')
        return
    }

    try {
        await ElMessageBox.confirm('确定要取消登录吗？', '提示', {
            confirmButtonText: '确定',
            cancelButtonText: '继续登录',
            type: 'warning'
        })
        done()
    } catch (error) {
        // 用户点击了取消，不关闭对话框
    }
}

// 切换用户展开状态
const toggleUserExpanded = (userUid: number) => {
    userConfigStore.toggleUserExpanded(userUid)
}

const {
    typeList,
    typeListV2,
    selectedCategory,
    selectedSubCategory,
    categoryPopoverVisible,
    isEditingTemplateName,
    editingTemplateName,
    templateNameInputRef,
    selectTemplate,
    resetTemplate,
    saveTemplate,
    reloadTemplateFromAV,
    handleTemplateCommand,
    handleTemplateCreated,
    handleTemplateNameEdit,
    saveTemplateName,
    cancelEditTemplateName,
    refreshSeasonList,
    onCategoryChange,
    onSubCategoryChange
} = useTemplateManager({
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
    saveTemplateSelection: (uid, name) => saveTemplateSelection(uid, name),
    clearSavedSelection: () => clearSavedSelection()
})

const { clearCardContent } = useCardContent({
    currentForm,
    tags,
    tagViewRef,
    selectedCategory,
    selectedSubCategory,
    templateLoading,
    selectedUser,
    getUserWatermark: uid => userConfigStore?.configRoot?.config[uid]?.watermark || 0,
    showMessage: (message, type) => utilsStore.showMessage(message, type)
})

// 使用 Tauri 文件对话框选择视频文件
// 处理文件夹监控提交稿件事件
const handleSubmitTemplate = async (
    mode: 'single' | 'multi' = 'single',
    options?: { auto?: boolean }
) => {
    if (mode === 'multi') {
        await submitTemplateAsSeparatePosts({
            skipConfirm: Boolean(options?.auto),
            autoTrigger: Boolean(options?.auto)
        })
        return
    }

    await submitTemplate()
}

// 自动开始待处理的任务
const autoStartWaitingTasks = async () => {
    if (!userConfigStore.configRoot?.auto_start) {
        return
    }

    // 刷新上传队列获取最新状态
    await uploadStore.getUploadQueue()

    // 获取所有待处理的任务
    const pendingTasks = uploadStore.uploadQueue.filter(task => task.status === 'Waiting')

    for (const task of pendingTasks) {
        try {
            await uploadStore.startUpload(task.id)
            console.log(`自动开始任务: ${task.id}`)
        } catch (error) {
            console.error(`自动开始任务失败 ${task.id}:`, error)
            // 继续处理下一个任务
        }
    }
}

// 检查是否所有文件都已上传完成
const allFilesUploaded = computed(() => {
    if (!currentForm.value?.videos || currentForm.value.videos.length === 0) {
        return false
    }
    return currentForm.value.videos.every(video => video.complete && video.path === '')
})

// 提交视频
const submitTemplate = async () => {
    if (!currentTemplateName.value || !selectedUser.value) {
        utilsStore.showMessage('请选择模板', 'error')
        return
    }

    if (!allFilesUploaded.value) {
        const currentAutoSubmitting = getCurrentAutoSubmitting.value
        if (!currentAutoSubmitting) {
            // 首次点击，开始自动提交
            // 将当前video列表加入upload queue
            try {
                if (currentForm.value?.videos && currentForm.value.videos.length > 0) {
                    await uploadStore.createUploadTask(
                        selectedUser.value.uid,
                        currentTemplateName.value,
                        currentForm.value.videos
                    )

                    setTimeout(async () => {
                        try {
                            await autoStartWaitingTasks()
                        } catch (error) {
                            console.error('自动开始任务失败:', error)
                        }
                    }, 500)
                }
            } catch (error) {
                console.error('添加到上传队列失败:', error)
                utilsStore.showMessage(`添加到上传队列失败: ${error}`, 'error')
            }
            setAutoSubmitting(selectedUser.value.uid, currentTemplateName.value, true)
            void checkAutoSubmitAll()
            utilsStore.showMessage('已启动自动提交，上传完成后将自动提交', 'info')
        } else {
            // 第二次点击，取消自动提交
            setAutoSubmitting(selectedUser.value.uid, currentTemplateName.value, false)
            utilsStore.showMessage('已取消自动提交', 'info')
        }
        return
    } else {
        performTemplateSubmit(selectedUser.value.uid, currentTemplateName.value, currentForm.value)
    }
}

const {
    userConfigVisible,
    configUser,
    hasActiveUploadTasks,
    syncCompletedTasksFromQueue,
    openUserConfig,
    handleLogoutUser
} = useUserActions({
    selectedUser,
    currentTemplateName,
    showLoginDialog,
    refreshAllData: () => refreshAllData(),
    clearSavedSelection: () => clearSavedSelection(),
    clearSavedSelectionForUser: uid => clearSavedSelectionForUser(uid)
})

// 刷新所有数据的方法
const refreshAllData = async () => {
    try {
        // 重新获取登录用户
        await authStore.getLoginUsers()
        // 重新构建用户模板
        await userConfigStore.ensureUserTemplatesReady()
        // 拉取最新的分区列表
        if (loginUsers.value && loginUsers.value.length > 0) {
            await utilsStore.initArchievePre(loginUsers.value[0].uid)
        }
        // 重新加载用户配置
        await userConfigStore.loadConfig()
        // 重写
        await userConfigStore.saveConfig()
    } catch (error) {
        console.error('刷新数据失败:', error)
    }
}

// 导出日志
const exportLogs = async () => {
    try {
        const zipPath = await utilsStore.exportLogs()
        const zipName = zipPath.split(/[/\\]/).pop() || zipPath

        const savePath = await save({
            defaultPath: zipName,
            filters: [{ name: 'ZIP', extensions: ['zip'] }]
        })

        if (savePath) {
            // 复制 ZIP 文件到用户指定位置
            await copyFile(zipPath, savePath)
            await remove(zipPath)
            console.log('文件已保存到：', savePath)
        }
    } catch (error) {
        console.error('导出日志失败:', error)
    }
}

// 检查视频转码状态
const checkVideoStatus = async () => {
    if (!selectedUser.value || !currentTemplate.value?.aid) return

    try {
        // 先刷新模板数据
        await ElMessageBox.confirm(
            `此操作会重新拉取模板数据，此操作会丢失未保存的更改，是否继续？`,
            '',
            {
                confirmButtonText: '刷新并继续',
                cancelButtonText: '不刷新，仅显示当前',
                type: 'info'
            }
        )
        await reloadTemplateFromAV(selectedUser.value.uid, currentTemplate.value.aid)
        // 然后显示状态对话框
        showVideoStatusDialog.value = true
    } catch (error) {
        console.error('刷新模板数据失败:', error)
        // 即使刷新失败也显示对话框
        showVideoStatusDialog.value = true
    }
}

// 检查更新
const checkUpdate = async () => {
    try {
        const updateInfo = await utilsStore.checkUpdate()
        if (updateInfo) {
            // 如果有更新，显示确认对话框
            try {
                await ElMessageBox.confirm(`发现新版本 ${updateInfo}，是否前往下载？`, '发现更新', {
                    confirmButtonText: '前往下载',
                    cancelButtonText: '稍后再说',
                    type: 'info'
                })
                // 用户确认后打开下载页面
                await openUrl(`https://github.com/biliup/biliup-app-new/releases/tag/${updateInfo}`)
            } catch {
                // 用户取消，不做任何操作
            }
        } else {
            utilsStore.showMessage('当前已是最新版本', 'success')
        }
    } catch (error) {
        console.error('检查更新失败:', error)
    }
}
</script>

<style scoped>
.main-view {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.header {
    background: #fff;
    border-bottom: 1px solid #e4e7ed;
    padding: 0 20px;
    position: sticky;
    top: 0;
    z-index: 100;
    flex-shrink: 0;
}

.header-content {
    display: flex;
    justify-content: space-between;
    align-items: center;
    height: 100%;
}

.app-title {
    margin: 0;
    color: #303133;
    display: inline-block;
}

.app-version {
    display: inline-block;
}

.header-center {
    display: flex;
    align-items: center;
    gap: 12px;
}

.header-right {
    display: flex;
    align-items: center;
    gap: 20px;
}

.global-config-btn {
    margin-right: 12px;
}

.main-container {
    flex: 1;
    overflow: hidden;
}

.main-content {
    padding: 0;
    overflow: hidden;
}

.content-wrapper {
    height: 100%;
    padding: 20px;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: #c1c1c1 transparent;
}

.content-wrapper::-webkit-scrollbar {
    width: 6px;
}

.content-wrapper::-webkit-scrollbar-track {
    background: transparent;
}

.content-wrapper::-webkit-scrollbar-thumb {
    background-color: #c1c1c1;
    border-radius: 3px;
}

.content-wrapper::-webkit-scrollbar-thumb:hover {
    background-color: #a8a8a8;
}

.no-selection,
.no-template {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
}

.form-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
    padding-bottom: 15px;
    border-bottom: 1px solid #e4e7ed;
}

.form-header h3 {
    margin: 0;
    color: #303133;
}

.template-name-container {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    flex: 1;
    margin-right: 20px;
}

.edit-bv-template-disaplay {
    display: inline-block;
}

.template-name-display {
    margin: 0;
    color: #303133;
    cursor: pointer;
    padding: 8px 12px;
    border-radius: 4px;
    transition: all 0.3s;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    position: relative;
    max-width: fit-content;
}

.template-name-display:hover {
    background: #f0f9ff;
    color: #409eff;
}

.template-name-display .edit-hint-icon {
    opacity: 0;
    font-size: 14px;
    transition: opacity 0.3s;
}

.template-name-display:hover .edit-hint-icon {
    opacity: 1;
}

.template-name-input {
    max-width: 300px;
}

.archive-state-badge {
    display: inline-flex;
    align-items: center;
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 12px;
    line-height: 1.4;
    color: #fff;
    white-space: nowrap;
}

.archive-state-badge.state-ok {
    background: #67c23a;
}

.archive-state-badge.state-neutral {
    background: #909399;
}

.archive-state-badge.state-error {
    background: #f56c6c;
}

.refresh-btn {
    cursor: pointer;
    color: #606266;
    font-size: 16px;
    transition: all 0.3s;
    border-radius: 4px;
}

.refresh-btn:hover {
    color: #409eff;
    background-color: #f0f9ff;
    transform: rotate(180deg);
}

.header-actions {
    display: flex;
    gap: 10px;
}

.form-section {
    margin-bottom: 20px;
}

.form-section.drag-target {
    border: 2px dashed #409eff;
    background: rgba(64, 158, 255, 0.05);
    transition: all 0.3s ease;
}

.form-section.drag-target .el-card__header {
    background: rgba(64, 158, 255, 0.1);
}

/* 卡片折叠样式 */
.card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    cursor: pointer;
    user-select: none;
    transition: all 0.3s ease;
    height: 10px;
}

.card-header:hover {
    color: #409eff;
}

.card-header .header-actions {
    display: flex;
    gap: 8px;
    align-items: center;
}

.card-header .header-actions .el-button {
    margin: 0;
    padding: 4px;
    border: none;
    background: transparent;
    transition: all 0.3s ease;
}

.card-header .header-actions .el-button:hover {
    background: rgba(245, 108, 108, 0.1);
    color: #f56c6c;
    transform: scale(1.1);
}

.card-header .header-actions .el-button .el-icon {
    font-size: 14px;
}

.collapse-icon {
    transition: transform 0.3s ease;
    color: #909399;
}

.collapse-icon:hover {
    color: #409eff;
}

.collapse-icon.collapsed {
    transform: rotate(-90deg);
}

.form-section.collapsed {
    margin-bottom: 10px;
}

.card-content {
    padding-top: 0;
}

.drag-hint {
    float: right;
    color: #409eff;
    font-size: 12px;
    font-weight: 500;
}

.upload-tip {
    color: #909399;
    font-size: 12px;
    margin-top: 5px;
}

.video-buttons-group {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-bottom: 10px;
}

.drag-active-tip {
    color: #409eff !important;
    font-weight: 500;
    animation: pulse 1.5s infinite;
}

@keyframes pulse {
    0% {
        opacity: 1;
    }
    50% {
        opacity: 0.7;
    }
    100% {
        opacity: 1;
    }
}

/* 登录对话框样式 */
.login-dialog :deep(.el-dialog) {
    margin: 0;
    padding: 0;
    border-radius: 0;
    background: transparent;
    box-shadow: none;
    max-height: 90vh;
}

.login-dialog :deep(.el-dialog__header) {
    display: none;
}

.login-dialog :deep(.el-dialog__body) {
    padding: 0;
    max-height: 90vh;
    overflow: hidden;
}

.login-dialog-content {
    max-height: 90vh;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: #c1c1c1 transparent;
}

.login-dialog-content::-webkit-scrollbar {
    width: 6px;
}

.login-dialog-content::-webkit-scrollbar-track {
    background: transparent;
}

.login-dialog-content::-webkit-scrollbar-thumb {
    background-color: #c1c1c1;
    border-radius: 3px;
}

.login-dialog-content::-webkit-scrollbar-thumb:hover {
    background-color: #a8a8a8;
}

.login-dialog-content .login-view {
    min-height: auto;
    padding: 0;
    background: transparent;
}

.login-dialog-content .login-container {
    max-width: none;
    width: 100%;
    padding: 0;
}

.login-dialog-content .login-card {
    margin: 0;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
    border-radius: 16px;
}

.checkbox-group {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.interactive-setting-row {
    display: inline-flex;
    align-items: center;
    gap: 8px;
}

.interactive-help-icon {
    color: #909399;
    cursor: pointer;
    font-size: 14px;
    transition: color 0.2s ease;
}

.interactive-help-icon:hover {
    color: #606266;
}

.season-refresh-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
}

.season-refresh-row :deep(.season-selector) {
    flex: 1;
    min-width: 0;
}

.season-refresh-btn {
    flex-shrink: 0;
    background: transparent;
    border: none;
    box-shadow: none;
    color: #909399;
}

.season-refresh-btn:hover {
    background: transparent;
    color: #409eff;
}

.season-refresh-btn:disabled {
    background: transparent;
    color: #c0c4cc;
}

/* 表单提示样式 */
.form-tip {
    font-size: 12px;
    color: #909399;
    margin-top: 5px;
    line-height: 1.4;
}

.form-tip div {
    margin-bottom: 2px;
}

/* 分区选择器样式 */
.category-trigger {
    width: 100%;
    display: flex !important;
    justify-content: space-between !important;
    align-items: center !important;
    border: 1px solid #dcdfe6;
    background: #fff;
    color: #606266;
    padding: 8px 15px;
    border-radius: 4px;
    cursor: pointer;
    transition: border-color 0.3s;
    position: relative;
}

.category-trigger .category-text {
    flex: 1;
    text-align: left;
    padding-right: 30px; /* 为右侧箭头留出空间 */
}

.category-trigger:hover {
    border-color: #409eff;
}

.category-trigger.el-button--primary {
    background: #fff;
    border-color: #409eff;
    color: #409eff;
}

.category-trigger .placeholder {
    color: #c0c4cc;
}

.category-trigger .arrow-icon {
    position: absolute;
    right: 12px;
    top: 50%;
    transform: translateY(-50%);
    transition: transform 0.3s;
    flex-shrink: 0;
}

/* 分区选择面板 */
.category-selector-panel {
    display: flex;
    height: 360px;
    border-radius: 6px;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.category-list {
    width: 180px;
    background: #f8f9fa;
    border-right: 1px solid #e9ecef;
    overflow-y: auto;
}

.subcategory-list {
    flex: 1;
    background: #fff;
    overflow-y: auto;
}

.category-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    cursor: pointer;
    transition: all 0.3s;
    border-bottom: 1px solid #f0f2f5;
    font-size: 13px;
}

.category-item:hover {
    background: #e6f7ff;
    color: #1890ff;
}

.category-item.active {
    background: #1890ff;
    color: #fff;
}

.category-item.active .arrow-right {
    color: #fff;
}

.category-name {
    font-size: 13px;
}

.arrow-right {
    color: #c0c4cc;
    font-size: 12px;
    transition: color 0.3s;
}

.subcategory-item {
    padding: 12px 16px;
    cursor: pointer;
    transition: all 0.3s;
    border-bottom: 1px solid #f0f2f5;
}

.subcategory-item:hover {
    background: #f0f9ff;
}

.subcategory-item.active {
    background: #e6f7ff;
    border-left: 3px solid #1890ff;
}

.subcategory-content {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.subcategory-name {
    font-size: 14px;
    font-weight: 500;
    color: #303133;
}

.subcategory-desc {
    font-size: 12px;
    color: #909399;
    line-height: 1.4;
}

.empty-subcategory {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
}

/* 滚动条样式 */
.category-list::-webkit-scrollbar,
.subcategory-list::-webkit-scrollbar {
    width: 6px;
}

.category-list::-webkit-scrollbar-track,
.subcategory-list::-webkit-scrollbar-track {
    background: transparent;
}

.category-list::-webkit-scrollbar-thumb,
.subcategory-list::-webkit-scrollbar-thumb {
    background-color: #c1c1c1;
    border-radius: 3px;
}

.category-list::-webkit-scrollbar-thumb:hover,
.subcategory-list::-webkit-scrollbar-thumb:hover {
    background-color: #a8a8a8;
}

/* 上传操作区域 */
.upload-actions {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 16px;
    padding: 20px 0;
    margin-top: 20px;
    border-top: 1px solid #e4e7ed;
}

.upload-actions .el-button {
    min-width: 140px;
}

.multi-submit-entry {
    display: inline-flex;
    align-items: center;
    gap: 10px;
}

.multi-submit-help-icon {
    color: #909399;
    cursor: pointer;
    font-size: 14px;
    transition: color 0.2s ease;
}

.multi-submit-help-icon:hover {
    color: #606266;
}

/* 拖拽覆盖层样式 */
.drag-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(64, 158, 255, 0.9);
    z-index: 9999;
    display: flex;
    align-items: center;
    justify-content: center;
    backdrop-filter: blur(4px);
}

.drag-content {
    text-align: center;
    color: white;
    padding: 40px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.1);
    border: 2px dashed rgba(255, 255, 255, 0.8);
    max-width: 500px;
}

.drag-icon {
    font-size: 64px;
    margin-bottom: 20px;
    animation: bounce 2s infinite;
}

@keyframes bounce {
    0%,
    20%,
    50%,
    80%,
    100% {
        transform: translateY(0);
    }
    40% {
        transform: translateY(-10px);
    }
    60% {
        transform: translateY(-5px);
    }
}

.drag-content h3 {
    margin: 0 0 10px 0;
    font-size: 24px;
    font-weight: 600;
}

.drag-content p {
    margin: 8px 0;
    font-size: 16px;
    opacity: 0.9;
}

.drag-content .warning-text {
    color: #ffd700;
    font-weight: 500;
    margin-top: 15px;
}

.template-name-display.disabled {
    cursor: not-allowed !important;
    opacity: 0.6 !important;
    color: #909399 !important;
}

.template-name-display.disabled .edit-hint-icon {
    color: #c0c4cc !important;
}
</style>

<style>
/* 全局样式：分区选择器popover */
.category-popover {
    padding: 0 !important;
}

.category-popover .el-popover__arrow {
    display: none;
}

.interactive-confirm-dialog {
    display: flex;
    flex-direction: column;
    gap: 10px;
}

.interactive-confirm-dialog-text {
    line-height: 1.6;
}

.interactive-confirm-dialog-checkbox {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
}

.interactive-confirm-dialog-checkbox input {
    cursor: pointer;
}
</style>
