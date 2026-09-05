<template>
    <div class="video-list-container">
        <!-- 视频操作按钮组 -->
        <div class="video-buttons-group">
            <el-button type="primary" @click="$emit('selectVideo')" size="small">
                <el-icon><upload-filled /></el-icon>
                选择视频文件
            </el-button>
            <el-button type="info" @click="showFolderWatchDialog = true" size="small">
                <el-icon><folder-opened /></el-icon>
                文件夹监控
            </el-button>
            <el-button
                type="success"
                size="small"
                :loading="props.uploading"
                @click="$emit('createUpload')"
                :disabled="!videos || videos.length === 0"
            >
                <el-icon><upload-filled /></el-icon>
                加入上传队列
            </el-button>
            <el-button
                type="danger"
                plain
                @click="$emit('clearAllVideos')"
                size="small"
                :disabled="!videos || videos.length === 0"
            >
                <el-icon><delete /></el-icon>
                清空{{ videos && videos.length > 0 ? `(${videos.length})` : '' }}
            </el-button>
        </div>
        <div class="upload-tip">
            <span v-if="!isDragOver"> 支持 MP4、AVI、MOV、MKV、WMV、FLV、M4V、WEBM 等格式 </span>
            <span v-else class="drag-active-tip"> 💡 松开鼠标即可添加文件到当前模板 </span>
        </div>

        <!-- 已上传文件列表 -->
        <div v-if="videos && videos.length > 0" class="uploaded-videos-section">
            <div class="uploaded-videos-list">
                <div
                    v-for="(video, index) in updatedVideos"
                    :key="video.id"
                    class="uploaded-video-item"
                    :class="getVideoWarningClass(video)"
                    :title="getVideoWarningTooltip(video)"
                >
                    <!-- 序号输入框 -->
                    <div class="video-order">
                        <el-input-number
                            :model-value="index + 1"
                            :min="1"
                            :max="updatedVideos.length"
                            size="small"
                            controls-position="right"
                            :step="-1"
                            @change="(newOrder: number) => handleReorderVideo(index, newOrder - 1)"
                            class="order-input"
                        />
                    </div>

                    <div class="video-status-icon">
                        <!-- 上传完成 -->
                        <el-icon v-if="video.status === 'Completed'" class="status-complete">
                            <circle-check />
                        </el-icon>
                        <!-- 上传中 -->
                        <el-icon v-else-if="video.status === 'Running'" class="status-uploading">
                            <loading />
                        </el-icon>
                        <!-- 失败 -->
                        <el-icon v-else-if="video.status === 'Failed'" class="status-failed">
                            <circle-close />
                        </el-icon>
                        <!-- 暂停 -->
                        <el-icon v-else-if="video.status === 'Paused'" class="status-paused">
                            <video-pause />
                        </el-icon>
                        <!-- 已取消 -->
                        <el-icon v-else-if="video.status === 'Cancelled'" class="status-cancelled">
                            <circle-close />
                        </el-icon>
                        <!-- 待上传/等待中 -->
                        <el-icon v-else class="status-pending">
                            <cloudy />
                        </el-icon>
                    </div>
                    <div class="video-info">
                        <!-- 文件名和状态在同一行 -->
                        <div class="video-title-row">
                            <div class="video-title-container">
                                <div v-if="editingFileId === video.id" class="video-title-edit">
                                    <el-input
                                        v-model="editingTitle"
                                        size="small"
                                        @keyup.enter="saveVideoTitle(video.id)"
                                        @blur="saveVideoTitle(video.id)"
                                        @keyup.esc="cancelEditVideoTitle"
                                        ref="videoTitleInput"
                                        maxlength="80"
                                    />
                                </div>
                                <div
                                    v-else
                                    class="video-title"
                                    @click="
                                        startEditVideoTitle(
                                            video.id,
                                            video.title || video.videoname
                                        )
                                    "
                                >
                                    <span class="video-title-text">{{
                                        video.title || video.videoname
                                    }}</span>
                                    <el-icon class="edit-icon"><edit /></el-icon>
                                    <svg
                                        :class="['ai-icon', { 'is-generating': aiGenerating }]"
                                        viewBox="0 0 16 16"
                                        xmlns="http://www.w3.org/2000/svg"
                                        :title="aiGenerating ? 'AI 正在生成标题…' : 'AI 一键生成标题（截取视频最后3秒画面）'"
                                        @click.stop.prevent="handleAiGenerateTitle(video)"
                                    >
                                        <path d="M0 0h16v16H0z" fill="none" />
                                        <path
                                            fill="currentColor"
                                            d="M5.465 9.83a.92.92 0 0 0 1.07 0a1 1 0 0 0 .341-.46l.347-1.067a1.7 1.7 0 0 1 1.078-1.078l1.086-.354a.923.923 0 0 0-.037-1.75l-1.069-.346a1.7 1.7 0 0 1-1.08-1.078l-.353-1.084a.92.92 0 0 0-.869-.61a.92.92 0 0 0-.875.624l-.356 1.09A1.71 1.71 0 0 1 3.7 4.775l-1.084.351a.923.923 0 0 0 .013 1.745l1.067.347a1.71 1.71 0 0 1 1.081 1.083l.352 1.08a.92.92 0 0 0 .337.449M4.007 6.264L3.152 6l.864-.28a2.7 2.7 0 0 0 1.045-.66a2.76 2.76 0 0 0 .644-1.056l.265-.859l.28.862a2.7 2.7 0 0 0 1.718 1.715l.88.27l-.86.28A2.7 2.7 0 0 0 6.27 7.986l-.265.857l-.279-.859a2.7 2.7 0 0 0-1.719-1.72m6.527 7.587A.8.8 0 0 0 11 14a.81.81 0 0 0 .759-.55l.248-.761a1.09 1.09 0 0 1 .68-.681l.772-.252a.8.8 0 0 0-.023-1.52l-.764-.25a1.08 1.08 0 0 1-.68-.678l-.252-.774a.8.8 0 0 0-1.518.011l-.247.762a1.07 1.07 0 0 1-.664.679l-.776.253a.8.8 0 0 0-.388 1.222c.099.14.239.244.4.3l.763.247a1.06 1.06 0 0 1 .68.683l.253.774a.8.8 0 0 0 .292.387m-.914-2.793L9.442 11l.184-.064a2.09 2.09 0 0 0 1.3-1.317l.058-.178l.06.181a2.08 2.08 0 0 0 1.316 1.316l.195.064l-.18.059a2.08 2.08 0 0 0-1.317 1.32l-.059.181l-.058-.18a2.07 2.07 0 0 0-1.32-1.322"
                                        />
                                    </svg>
                                </div>
                            </div>

                            <!-- 状态标签移动到文件名右侧 -->
                            <div class="video-status">
                                <span
                                    class="status-text"
                                    :class="{
                                        complete: video.status === 'Completed',
                                        uploading: video.status === 'Running',
                                        pending:
                                            video.status === 'Waiting' ||
                                            video.status === 'Pending',
                                        failed: video.status === 'Failed',
                                        paused: video.status === 'Paused',
                                        cancelled: video.status === 'Cancelled'
                                    }"
                                >
                                    {{ getStatusText(video.status || 'Waiting') }}
                                </span>
                                <span
                                    v-if="publishTimeTexts[index]"
                                    class="publish-time"
                                    :title="`预计发布：${publishTimeTexts[index]}`"
                                >
                                    {{ publishTimeTexts[index] }}
                                </span>
                            </div>
                        </div>

                        <!-- 每个视频独立封面设置 -->
                        <div class="video-cover-row">
                            <CoverUploader
                                :model-value="video.cover || ''"
                                :title="video.title || video.videoname"
                                :uid="uid"
                                :disabled="disabled"
                                @update:model-value="
                                    (val: string) => handleVideoCoverChange(video.id, val)
                                "
                            />
                        </div>

                        <!-- 进度条区域 -->
                        <div class="progress-section">
                            <div
                                class="progress-bar-container"
                                v-if="video.status !== 'Completed' && video.status !== 'Failed'"
                            >
                                <el-progress
                                    :percentage="video.progress"
                                    :show-text="false"
                                    size="small"
                                    :stroke-width="3"
                                    :color="getProgressColor(video.status)"
                                />
                                <span class="progress-text"
                                    >{{ formatUploadProgress(video) }}%</span
                                >
                            </div>
                            <div v-if="video.status === 'Failed'" class="error-message">
                                {{ video.errorMessage || '上传失败' }}
                            </div>
                            <div
                                class="upload-speed"
                                v-if="video.status === 'Running' && video.speed > 0"
                            >
                                {{ formatUploadSpeed(video) }}
                            </div>
                        </div>
                        <!-- 完成时间显示 -->
                        <span
                            class="completed-time"
                            v-if="video.status === 'Completed' && video.finished_at"
                        >
                            {{ formatFinishedTime(video.finished_at) }}
                        </span>
                    </div>

                    <!-- 文件操作按钮 -->
                    <div class="video-actions">
                        <el-button
                            type="danger"
                            size="small"
                            text
                            @click="handleRemoveFile(video.id)"
                        >
                            <el-icon><delete /></el-icon>
                        </el-button>
                    </div>
                </div>
            </div>

            <div class="batch-rename-tools">
                <div class="batch-rename-item">
                    <el-checkbox v-model="useUnifiedPrefix">添加前缀</el-checkbox>
                    <el-input
                        v-if="useUnifiedPrefix"
                        v-model="unifiedPrefixValue"
                        size="small"
                        class="batch-rename-input"
                        placeholder="输入前缀"
                        maxlength="80"
                    />
                </div>
                <div class="batch-rename-item">
                    <el-checkbox v-model="useUnifiedSuffix">添加后缀</el-checkbox>
                    <el-input
                        v-if="useUnifiedSuffix"
                        v-model="unifiedSuffixValue"
                        size="small"
                        class="batch-rename-input"
                        placeholder="输入后缀"
                        maxlength="80"
                    />
                </div>
                <div class="batch-rename-item schedule-item">
                    <span class="schedule-label">定时发布</span>
                    <el-date-picker
                        v-model="scheduleStartDate"
                        type="date"
                        size="small"
                        placeholder="选择开始日期"
                        value-format="YYYY-MM-DD"
                        :clearable="false"
                        class="schedule-date-picker"
                    />
                    <el-input-number
                        v-model="videosPerTimeSlot"
                        :min="1"
                        :max="99"
                        size="small"
                        controls-position="right"
                        class="schedule-count-input"
                    />
                    <span class="schedule-tip">个/时段</span>
                </div>
                <div class="sort-tools">
                    <el-button
                        size="small"
                        @click="sortVideosByTitle('asc')"
                        :disabled="!videos || videos.length < 2"
                    >
                        标题正序
                    </el-button>
                    <el-button
                        size="small"
                        @click="sortVideosByTitle('desc')"
                        :disabled="!videos || videos.length < 2"
                    >
                        标题倒序
                    </el-button>
                </div>
            </div>
        </div>

        <!-- 文件夹监控对话框 -->
        <FloderWatch
            v-model="showFolderWatchDialog"
            :current-videos="updatedVideos"
            :template-title="templateTitle"
            @add-videos="handleAddVideos"
            @submit-videos="handleSubmitVideos"
        />
    </div>
</template>

<script setup lang="ts">
import { ref, nextTick, computed, onMounted, onUnmounted, watch } from 'vue'
import {
    CircleCheck,
    Loading,
    Cloudy,
    Edit,
    Delete,
    UploadFilled,
    FolderOpened,
    CircleClose,
    VideoPause
} from '@element-plus/icons-vue'
import { useUploadStore } from '../stores/upload'
import { useUserConfigStore } from '../stores/user_config'
import { useUtilsStore } from '../stores/utils'
import FloderWatch from './FloderWatch.vue'
import CoverUploader from './CoverUploader.vue'
import { isVideoReadyForSeparateSubmit, getSeparateSubmitBlockReason } from '../utils/videoSubmit'

// Props
interface Props {
    videos: any[]
    isDragOver?: boolean
    uploading?: boolean
    templateTitle?: string
    uid?: number
    disabled?: boolean
    /** LastPublishedBadge 计算出的上次发布时间（Unix 秒），作为排期起始基准 */
    lastPublishTime?: number
    /** 是否处于多稿件提交模式（用于提示未就绪视频） */
    separateSubmitting?: boolean
}

const props = withDefaults(defineProps<Props>(), {
    isDragOver: false,
    uploading: false,
    disabled: false,
    lastPublishTime: 0,
    separateSubmitting: false
})

// Emits
const emit = defineEmits<{
    'update:videos': [videos: any[]]
    selectVideo: []
    clearAllVideos: []
    removeFile: [id: string]
    createUpload: []
    addVideosToForm: [videos: any[]]
    submitTemplate: [mode?: 'single' | 'multi', options?: { auto?: boolean }]
}>()

// 文件编辑状态
const editingFileId = ref<string | null>(null)
const editingTitle = ref('')
const videoTitleInput = ref()
const uploadStore = useUploadStore()
const userConfigStore = useUserConfigStore()
const utilsStore = useUtilsStore()

// AI 标题生成状态（生成单个标题后自动应用）
const aiGenerating = ref(false)

// 文件夹监控对话框状态
const showFolderWatchDialog = ref(false)

// 添加前缀 / 后缀：按「uid + 模板名」为维度隔离各模板的独立状态，
// 避免切换模板时把上一个模板的前缀/后缀串用（自动应用）到其他模板上
interface AffixState {
    usePrefix: boolean
    prefixValue: string
    useSuffix: boolean
    suffixValue: string
    lastAppliedPrefix: string
    lastAppliedSuffix: string
}
const createEmptyAffixState = (): AffixState => ({
    usePrefix: false,
    prefixValue: '',
    useSuffix: false,
    suffixValue: '',
    lastAppliedPrefix: '',
    lastAppliedSuffix: ''
})
const useUnifiedPrefix = ref(false)
const useUnifiedSuffix = ref(false)
const unifiedPrefixValue = ref('')
const unifiedSuffixValue = ref('')
const lastAppliedPrefix = ref('')
const lastAppliedSuffix = ref('')

const affixStateMap = new Map<string, AffixState>()
// 模板切换（保存/恢复状态）期间置为 true，抑制各 watch 的连锁副作用
let suppressAffixSideEffects = false

const getAffixTemplateKey = (): string =>
    `${props.uid ?? ''}::${String(props.templateTitle || '').trim()}`

const collectAffixState = (): AffixState => ({
    usePrefix: useUnifiedPrefix.value,
    prefixValue: unifiedPrefixValue.value,
    useSuffix: useUnifiedSuffix.value,
    suffixValue: unifiedSuffixValue.value,
    lastAppliedPrefix: lastAppliedPrefix.value,
    lastAppliedSuffix: lastAppliedSuffix.value
})

const applyAffixState = (state: AffixState) => {
    useUnifiedPrefix.value = state.usePrefix
    useUnifiedSuffix.value = state.useSuffix
    unifiedPrefixValue.value = state.prefixValue
    unifiedSuffixValue.value = state.suffixValue
    lastAppliedPrefix.value = state.lastAppliedPrefix
    lastAppliedSuffix.value = state.lastAppliedSuffix
}

const saveAffixState = (key: string) => {
    if (!key) return
    affixStateMap.set(key, collectAffixState())
}

const restoreAffixState = (key: string) => {
    suppressAffixSideEffects = true
    applyAffixState(affixStateMap.get(key) || createEmptyAffixState())
    // pre 队列的 watch 会先于 nextTick 触发，故在下一轮再恢复副作用
    nextTick(() => {
        suppressAffixSideEffects = false
    })
}

// 定时发布时间排布配置
const TIME_SLOTS = [9, 12, 18, 23] // 每天发布的时间点：9点、12点、18点、23点
const scheduleStartDate = ref('')
const videosPerTimeSlot = ref(1)

// 默认开始日期为今天（本地时区）
const now = new Date()
scheduleStartDate.value = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`

// 每个时间点的视频数量（至少为1）
const getVideosPerTimeSlot = () => Math.max(1, Math.floor(Number(videosPerTimeSlot.value)) || 1)

// 简单字符串哈希，作为随机种子
const hashSeed = (str: string): number => {
    let h = 0
    for (let i = 0; i < str.length; i++) {
        h = (h * 31 + str.charCodeAt(i)) >>> 0
    }
    return h
}

// 基于种子的伪随机数生成器（mulberry32），保证同一配置下时间稳定不跳动
const mulberry32 = (seed: number) => {
    let a = seed >>> 0
    return () => {
        a |= 0
        a = (a + 0x6d2b79f5) | 0
        let t = Math.imul(a ^ (a >>> 15), 1 | a)
        t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
        return ((t ^ (t >>> 14)) >>> 0) / 4294967296
    }
}

const pad2 = (n: number) => String(n).padStart(2, '0')

// 排期计算的起始基准（毫秒时间戳）
// 取 手动选择的开始日期、LastPublishedBadge 计算出的上次发布时间、当前时间 三者的最大值，
// 保证新排期一定晚于上次发布时间；当上次发布时间早于当前时间时，以当前时间为起始基准
const effectiveStartMs = computed(() => {
    // 兼容字符串（YYYY-MM-DD）与 Date 对象两种取值，取当天零点
    const startDateRaw = scheduleStartDate.value as unknown
    let startMs = 0
    if (startDateRaw) {
        const startDate: Date | string =
            startDateRaw instanceof Date ? startDateRaw : String(startDateRaw)
        const start = new Date(startDate instanceof Date ? startDate : `${startDate}T00:00:00`)
        if (!isNaN(start.getTime())) startMs = start.getTime()
    }
    const lastPublishMs = (props.lastPublishTime || 0) * 1000
    const nowMs = Date.now()
    return Math.max(startMs, lastPublishMs, nowMs)
})

// 计算某个（日期, 时段, 位置）对应的具体发布时间
const getSlotTime = (day: Date, perSlot: number, slotIndex: number, posInSlot: number): Date => {
    const year = day.getFullYear()
    const month = day.getMonth() + 1
    const dayOfMonth = day.getDate()

    // 以 (日期, 时段, 每时段数量) 为种子，保证切换数量时时间也会重新随机生成
    const rand = mulberry32(hashSeed(`${year}-${month}-${dayOfMonth}-${slotIndex}-${perSlot}`))
    const offsets: number[] = []
    for (let i = 0; i < perSlot; i++) {
        offsets.push(Math.floor(rand() * 90) - 45) // -45 ~ +44 分钟
    }
    offsets.sort((a, b) => a - b)

    const totalMinutes = TIME_SLOTS[slotIndex] * 60 + offsets[posInSlot]
    const hour = Math.floor(totalMinutes / 60)
    const minute = totalMinutes % 60

    const date = new Date(day)
    date.setHours(hour, minute, 0, 0)
    return date
}

// 按列表顺序计算某个位置的视频发布时间
// 以 effectiveStartMs 为起点：起始日当天早于/等于基准时间的时段全部跳过，
// 之后的视频按 每天 TIME_SLOTS.length 个时间点、每个时间点发布 videosPerTimeSlot 个 顺延排布，
// 保证整体从早到晚有序，同时时间点随机化（如 08:52、09:17）
const getPublishTimeByIndex = (index: number): Date | null => {
    const baseMs = effectiveStartMs.value
    if (!baseMs || isNaN(baseMs)) return null

    const perSlot = getVideosPerTimeSlot()
    const perDay = perSlot * TIME_SLOTS.length

    // 基准时间的当天零点作为起始日
    const anchorDay = new Date(baseMs)
    anchorDay.setHours(0, 0, 0, 0)

    // 起始日当天早于/等于基准时间的时段数量，全部跳过，保证发布时间晚于基准时间
    let shift = 0
    for (let slotIndex = 0; slotIndex < TIME_SLOTS.length; slotIndex++) {
        for (let posInSlot = 0; posInSlot < perSlot; posInSlot++) {
            if (getSlotTime(anchorDay, perSlot, slotIndex, posInSlot).getTime() <= baseMs) {
                shift++
            }
        }
    }

    const virtualIndex = index + shift
    const dayOffset = Math.floor(virtualIndex / perDay)
    const slotIndex = Math.min(Math.floor((virtualIndex % perDay) / perSlot), TIME_SLOTS.length - 1)
    const posInSlot = virtualIndex % perSlot

    const targetDay = new Date(anchorDay)
    targetDay.setDate(targetDay.getDate() + dayOffset)
    return getSlotTime(targetDay, perSlot, slotIndex, posInSlot)
}

// 格式化发布时间显示文本（MM-DD HH:mm）
const formatScheduleTime = (date: Date): string => {
    return `${pad2(date.getMonth() + 1)}-${pad2(date.getDate())} ${pad2(date.getHours())}:${pad2(date.getMinutes())}`
}

// 视频 id -> 稳定排期序号
// 新视频首次进入列表时按列表顺序分配序号，之后无论列表如何增删（如上传成功后从头移除），
// 已分配的视频发布时间都保持不变
const scheduleIndexMap = ref<Record<string, number>>({})

const syncScheduleIndexes = (list: any[]) => {
    const map = scheduleIndexMap.value
    const ids = list.map(v => String(v.id))
    const idSet = new Set(ids)

    // 清理已不在列表中的视频，释放序号
    for (const key of Object.keys(map)) {
        if (!idSet.has(key)) {
            delete map[key]
        }
    }

    // 为新增视频分配后续序号，保持它们在列表中的相对顺序
    const existing = Object.values(map)
    let nextIndex = existing.length > 0 ? Math.max(...existing) + 1 : 0
    for (const id of ids) {
        if (!(id in map)) {
            map[id] = nextIndex++
        }
    }
}

// 视频列表增删变化时同步排期序号
watch(
    () => props.videos.map(v => String(v.id)).join('|'),
    () => syncScheduleIndexes(props.videos),
    { immediate: true }
)

// 模板标题
const templateTitle = computed(() => props.templateTitle)

// 用于触发时间更新的响应式变量
const currentTime = ref(Date.now())
let timeUpdateTimer: ReturnType<typeof setInterval> | null = null

// 定时更新当前时间，用于相对时间的实时更新
onMounted(() => {
    timeUpdateTimer = setInterval(() => {
        currentTime.value = Date.now()
    }, 60000) // 每分钟更新一次
})

onUnmounted(() => {
    if (timeUpdateTimer) {
        clearInterval(timeUpdateTimer)
    }
})

// 实时更新的视频数据计算属性
const updatedVideos = computed(() => {
    if (!props.videos || props.videos.length === 0) return []

    let hasChanges = false
    const updatedList = props.videos.map(video => {
        const updatedVideo = { ...video }
        const originalVideo = { ...video }

        if (updatedVideo.id == updatedVideo.filename || !updatedVideo.path) {
            updatedVideo.complete = true
            updatedVideo.status = 'Completed'
            updatedVideo.errorMessage = ''
        } else {
            const task = uploadStore.getUploadTask(updatedVideo.id)
            if (task) {
                updatedVideo.complete = task.status === 'Completed'
                updatedVideo.status = task.status || 'Waiting'
                updatedVideo.errorMessage = task.error_message || ''
                updatedVideo.totalSize = task.total_size || 0
                updatedVideo.speed = task.speed || 0
                updatedVideo.progress = task.progress || 0
                updatedVideo.finished_at = task.finished_at || 0
                updatedVideo.cid = task.video.cid || 0
            } else {
                updatedVideo.complete = false
                updatedVideo.status = 'Waiting'
                updatedVideo.errorMessage = ''
                updatedVideo.totalSize = 0
                updatedVideo.speed = 0
                updatedVideo.progress = 0
                updatedVideo.finished_at = 0
            }
        }

        // 检查是否有变化
        if (
            originalVideo.complete !== updatedVideo.complete ||
            originalVideo.errorMessage !== updatedVideo.errorMessage ||
            originalVideo.status !== updatedVideo.status ||
            originalVideo.totalSize !== updatedVideo.totalSize ||
            originalVideo.speed !== updatedVideo.speed ||
            originalVideo.progress !== updatedVideo.progress ||
            originalVideo.finished_at !== updatedVideo.finished_at ||
            originalVideo.cid !== updatedVideo.cid
        ) {
            hasChanges = true
        }

        return updatedVideo
    })

    // 如果有变化，同步更新回 props.videos
    if (hasChanges) {
        // 使用 nextTick 确保在下一个事件循环中更新，避免无限循环
        nextTick(() => {
            emit('update:videos', updatedList)
        })
    }

    return updatedList
})

// 与 updatedVideos 顺序一一对应的发布时间(Date)列表
// 基于稳定排期序号（scheduleIndexMap）计算，列表缩短/增删不会导致已分配时间变动
const publishTimes = computed<(Date | null)[]>(() => {
    return updatedVideos.value.map(video => {
        const scheduleIndex = scheduleIndexMap.value[String(video.id)]
        return scheduleIndex === undefined ? null : getPublishTimeByIndex(scheduleIndex)
    })
})

// 发布时间的显示文本列表
const publishTimeTexts = computed<string[]>(() => {
    return publishTimes.value.map(date => (date ? formatScheduleTime(date) : ''))
})

// 根据发布时间列表回填 video 的 dtime 字段（Unix 秒），仅在实际变化时触发更新
const syncDtimeFromPublishTimes = (times: (Date | null)[]) => {
    if (!props.videos || props.videos.length === 0) return

    let hasChanged = false
    const newVideos = props.videos.map((video, index) => {
        const date = times[index]
        const dtime = date ? Math.floor(date.getTime() / 1000) : 0
        if (video.dtime !== dtime) {
            hasChanged = true
            return { ...video, dtime }
        }
        return video
    })

    if (hasChanged) {
        emit('update:videos', newVideos)
    }
}

// 监听发布时间变化，同步回填 dtime
watch(publishTimes, times => syncDtimeFromPublishTimes(times), { immediate: true })

// 切换日期或每时段数量后，强制重新计算并同步更新预计发布时间
watch([scheduleStartDate, videosPerTimeSlot], () => {
    syncDtimeFromPublishTimes(publishTimes.value)
})

// 重新排序视频
const handleReorderVideo = (currentIndex: number, newIndex: number) => {
    if (currentIndex === newIndex || newIndex < 0 || newIndex >= props.videos.length) {
        return
    }

    const newVideos = [...props.videos]
    const [movedItem] = newVideos.splice(currentIndex, 1)
    newVideos.splice(newIndex, 0, movedItem)

    emit('update:videos', newVideos)
}

const getVideoSortName = (video: any) => {
    return String(video.title || video.videoname || '').toLocaleLowerCase()
}

const sortVideosByTitle = (direction: 'asc' | 'desc') => {
    if (!props.videos || props.videos.length < 2) {
        return
    }

    const sortedVideos = [...props.videos].sort((a, b) => {
        const compareResult = getVideoSortName(a).localeCompare(getVideoSortName(b), 'zh-Hans-CN', {
            sensitivity: 'base',
            numeric: true
        })
        return direction === 'asc' ? compareResult : -compareResult
    })

    emit('update:videos', sortedVideos)
}

// 开始编辑视频标题
const startEditVideoTitle = (id: string, currentName: string) => {
    editingFileId.value = id
    editingTitle.value = currentName
    nextTick(() => {
        videoTitleInput.value[0].$el.querySelector('input').focus()
    })
}

// 保存视频标题
const saveVideoTitle = (id: string) => {
    if (!editingTitle.value.trim()) {
        cancelEditVideoTitle()
        return
    }

    const newVideos = props.videos.map(video => {
        if (video.id === id) {
            return {
                ...video,
                title: editingTitle.value.trim().slice(0, 80)
            }
        }
        return video
    })

    emit('update:videos', newVideos)
    cancelEditVideoTitle()
}

// 检查 AI 配置是否已就绪（启用且已填接口地址/Key/模型）
const isAiConfigured = (): boolean => {
    const ai = userConfigStore.configRoot?.ai
    return !!ai && !!ai.enabled && !!ai.api_key && !!ai.model
}

// 触发 AI 一键生成标题：截取视频倒数第三秒画面提交模型，成功后自动应用
const handleAiGenerateTitle = async (video: any) => {
    if (aiGenerating.value) {
        return
    }
    const localPath = video.original_file_path || video.path || ''
    if (!localPath) {
        utilsStore.showMessage('该视频缺少本地源文件，AI 生成标题仅支持本地视频', 'warning')
        return
    }
    if (!isAiConfigured()) {
        utilsStore.showMessage(
            '尚未配置 AI 服务，请先在「全局设置 → AI 设置」中开启并填写接口地址 / API Key / 模型',
            'warning'
        )
        return
    }
    aiGenerating.value = true
    try {
        const title = await utilsStore.generateAiTitle(localPath)
        const newTitle = String(title || '').trim().slice(0, 80)
        if (!newTitle) {
            utilsStore.showMessage('AI 未返回有效标题，请稍后重试', 'warning')
            return
        }
        const newVideos = props.videos.map(item => {
            if (item.id === video.id) {
                return {
                    ...item,
                    title: newTitle
                }
            }
            return item
        })
        emit('update:videos', newVideos)
        utilsStore.showMessage(`已应用 AI 标题：${newTitle}`, 'success')
    } catch (error) {
        utilsStore.showMessage(`AI 生成标题失败: ${error}`, 'error')
    } finally {
        aiGenerating.value = false
    }
}

// 更新单个视频的封面
const handleVideoCoverChange = (id: string, cover: string) => {
    const newVideos = props.videos.map(video => {
        if (video.id === id) {
            return {
                ...video,
                cover
            }
        }
        return video
    })
    emit('update:videos', newVideos)
}

const applyUnifiedNameAffixes = () => {
    if (!props.videos || props.videos.length === 0) {
        return
    }

    const nextPrefix = useUnifiedPrefix.value ? unifiedPrefixValue.value : ''
    const nextSuffix = useUnifiedSuffix.value ? unifiedSuffixValue.value : ''
    const previousPrefix = lastAppliedPrefix.value
    const previousSuffix = lastAppliedSuffix.value

    let hasChanged = false
    const updated = props.videos.map(video => {
        const currentName = String(video.title || video.videoname || '')
        let baseName = currentName

        if (previousPrefix && baseName.startsWith(previousPrefix)) {
            baseName = baseName.slice(previousPrefix.length)
        }

        if (previousSuffix && baseName.endsWith(previousSuffix)) {
            baseName = baseName.slice(0, baseName.length - previousSuffix.length)
        }

        const nextName = `${nextPrefix}${baseName}${nextSuffix}`.slice(0, 80)
        if (nextName !== video.title) {
            hasChanged = true
            return {
                ...video,
                title: nextName
            }
        }

        return video
    })

    lastAppliedPrefix.value = nextPrefix
    lastAppliedSuffix.value = nextSuffix

    if (hasChanged) {
        emit('update:videos', updated)
    }
}

const detectUnifiedAffixesFromVideos = () => {
    if (!props.videos || props.videos.length === 0) {
        return { prefix: '', suffix: '' }
    }

    const displayNames = props.videos.map(video => String(video.title || video.videoname || ''))
    if (displayNames.some(name => !name)) {
        return { prefix: '', suffix: '' }
    }

    const getLongestCommonPrefix = (names: string[]) => {
        if (names.length === 0) return ''
        let prefix = names[0]
        for (let i = 1; i < names.length; i++) {
            while (!names[i].startsWith(prefix) && prefix) {
                prefix = prefix.slice(0, -1)
            }
            if (!prefix) break
        }
        return prefix
    }

    const getLongestCommonSuffix = (names: string[]) => {
        if (names.length === 0) return ''
        let suffix = names[0]
        for (let i = 1; i < names.length; i++) {
            while (!names[i].endsWith(suffix) && suffix) {
                suffix = suffix.slice(1)
            }
            if (!suffix) break
        }
        return suffix
    }

    // 仅有一个视频时，宽松识别缺乏参照，不自动回填
    if (displayNames.length === 1) {
        return { prefix: '', suffix: '' }
    }

    // 宽松识别：基于当前标题集合的最长公共前缀/后缀
    let loosePrefix = getLongestCommonPrefix(displayNames)
    let looseSuffix = getLongestCommonSuffix(displayNames)

    const minLen = Math.min(...displayNames.map(name => name.length))
    if (loosePrefix.length + looseSuffix.length >= minLen) {
        // 出现重叠时优先裁掉后缀与前缀重复的部分，避免把有效后缀整体清空
        let overlap = loosePrefix.length + looseSuffix.length - minLen
        if (overlap > 0) {
            const suffixTrim = Math.min(overlap, looseSuffix.length)
            looseSuffix = looseSuffix.slice(suffixTrim)
            overlap -= suffixTrim
        }

        if (overlap > 0) {
            const prefixTrim = Math.min(overlap, loosePrefix.length)
            loosePrefix = loosePrefix.slice(0, loosePrefix.length - prefixTrim)
        }
    }

    return {
        prefix: loosePrefix,
        suffix: looseSuffix
    }
}

// 切换模板（或账号）时：存档当前模板的前/后缀状态，再恢复目标模板的状态。
// 该 watch 必须先于下方各 apply watch 注册，确保恢复完成后 videos 变化才会生效。
watch(getAffixTemplateKey, (newKey, oldKey) => {
    if (newKey === oldKey) return
    saveAffixState(oldKey)
    restoreAffixState(newKey)
})

watch([unifiedPrefixValue, unifiedSuffixValue], () => {
    if (suppressAffixSideEffects) return
    if (useUnifiedPrefix.value || useUnifiedSuffix.value) {
        applyUnifiedNameAffixes()
    }
})

watch(useUnifiedPrefix, enabled => {
    if (suppressAffixSideEffects) return
    if (enabled) {
        const { prefix } = detectUnifiedAffixesFromVideos()
        unifiedPrefixValue.value = prefix
        // 预置上次已应用值，避免勾选时把已存在前缀再次叠加
        lastAppliedPrefix.value = prefix

        applyUnifiedNameAffixes()
        return
    }

    // 取消勾选时仅隐藏输入，不改动已写入的视频标题
    lastAppliedPrefix.value = ''
})

watch(useUnifiedSuffix, enabled => {
    if (suppressAffixSideEffects) return
    if (enabled) {
        const { suffix } = detectUnifiedAffixesFromVideos()
        unifiedSuffixValue.value = suffix
        // 预置上次已应用值，避免勾选时把已存在后缀再次叠加
        lastAppliedSuffix.value = suffix

        applyUnifiedNameAffixes()
        return
    }

    // 取消勾选时仅隐藏输入，不改动已写入的视频标题
    lastAppliedSuffix.value = ''
})

watch(
    () =>
        props.videos.map(video => `${video.id}:${video.title || video.videoname || ''}`).join('|'),
    () => {
        if (suppressAffixSideEffects) return
        if (useUnifiedPrefix.value || useUnifiedSuffix.value) {
            applyUnifiedNameAffixes()
        }
    }
)

// 取消编辑视频标题
const cancelEditVideoTitle = () => {
    editingFileId.value = null
    editingTitle.value = ''
}

// 格式化上传进度
const formatUploadProgress = (video: any) => {
    return Math.round(video.progress || 0)
}

// 格式化上传速度
const formatUploadSpeed = (video: any) => {
    const speed = video.speed || 0
    if (speed < 1024) {
        return `${speed.toFixed(1)} B/s`
    } else if (speed < 1024 * 1024) {
        return `${(speed / 1024).toFixed(1)} KB/s`
    } else {
        return `${(speed / 1024 / 1024).toFixed(1)} MB/s`
    }
}

// 格式化完成时间
const formatFinishedTime = (timestamp: number | string): string => {
    try {
        const date = new Date(timestamp)
        const now = new Date(currentTime.value) // 使用响应式的当前时间
        const diffMs = now.getTime() - date.getTime()
        const diffMins = Math.floor(diffMs / (1000 * 60))
        const diffHours = Math.floor(diffMs / (1000 * 60 * 60))
        const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))

        if (diffMins < 1) return '刚刚完成'
        if (diffMins < 60) return `${diffMins}分钟前`
        if (diffHours < 24) return `${diffHours}小时前`
        if (diffDays < 7) return `${diffDays}天前`

        return ''
    } catch {
        return '未知时间'
    }
}

// 获取状态文本，与UploadQueue保持一致
const getStatusText = (status: string) => {
    const statusMap = {
        Waiting: '待开始',
        Pending: '等待中',
        Running: '上传中',
        Completed: '已完成',
        Cancelled: '已取消',
        Paused: '已暂停',
        Failed: '失败'
    }
    return statusMap[status as keyof typeof statusMap] || status
}

// 获取进度条颜色
const getProgressColor = (status: string) => {
    switch (status) {
        case 'Running':
            return '#409eff'
        case 'Failed':
            return '#f56c6c'
        case 'Paused':
            return '#e6a23c'
        case 'Cancelled':
            return '#909399'
        default:
            return '#409eff'
    }
}

// 检查视频是否超过8小时（需要警告）
const isVideoExpiredSoon = (video: any): boolean => {
    if (video.status !== 'Completed' || !video.finished_at) return false

    try {
        const finishedDate = new Date(video.finished_at)
        const now = new Date(currentTime.value) // 使用响应式的当前时间
        const diffHours = Math.floor((now.getTime() - finishedDate.getTime()) / (1000 * 60 * 60))

        return diffHours >= 8
    } catch {
        return false
    }
}

// 多稿件提交模式下，上传完成但未就绪（未改名/未设置封面）的视频会被阻塞提交
const isBlockedForSeparateSubmit = (video: any): boolean => {
    if (!props.separateSubmitting) return false
    if (!(video.complete && video.path === '')) return false
    return !isVideoReadyForSeparateSubmit(video)
}

// 获取视频警告样式类
const getVideoWarningClass = (video: any): string => {
    const classes: string[] = []

    if (isVideoExpiredSoon(video)) {
        classes.push('video-warning')
        try {
            const finishedDate = new Date(video.finished_at)
            const now = new Date(currentTime.value) // 使用响应式的当前时间
            const diffHours = (now.getTime() - finishedDate.getTime()) / (1000 * 60 * 60)
            if (diffHours >= 8) {
                classes.push('video-expired')
            }
        } catch {
            // 保持默认警告样式
        }
    }

    if (isBlockedForSeparateSubmit(video)) {
        classes.push('video-warning', 'video-rename-pending')
    }

    return classes.join(' ')
}

// 获取视频警告提示文本
const getVideoWarningTooltip = (video: any): string => {
    const tips: string[] = []

    if (isVideoExpiredSoon(video)) {
        try {
            const finishedDate = new Date(video.finished_at)
            const now = new Date(currentTime.value) // 使用响应式的当前时间
            const diffHours = Math.floor(
                (now.getTime() - finishedDate.getTime()) / (1000 * 60 * 60)
            )

            if (diffHours >= 10) {
                tips.push('此视频完成超过10小时，服务器可能已删除相关文件')
            } else {
                tips.push(`此视频完成已${diffHours}小时，服务器将在10小时后删除相关文件`)
            }
        } catch {
            tips.push('视频完成时间较长，可能无法上传')
        }
    }

    if (isBlockedForSeparateSubmit(video)) {
        tips.push(`待提交：${getSeparateSubmitBlockReason(video)}`)
    }

    return tips.join('；')
}

// 处理删除文件
const handleRemoveFile = (id: string) => {
    emit('removeFile', id)
}

// 处理文件夹监控添加视频
const handleAddVideos = (newVideos: any[]) => {
    // 发出添加视频事件到MainView，让它调用addVideoToCurrentForm处理每个视频
    emit('addVideosToForm', newVideos)
}

// 处理文件夹监控提交稿件
const handleSubmitVideos = (mode: 'single' | 'multi', options?: { auto?: boolean }) => {
    // 发出提交稿件事件到MainView，让它调用submitTemplate
    emit('submitTemplate', mode, options)
}
</script>

<style scoped>
.video-list-container {
    width: 100%;
}

.uploaded-videos-section {
    --video-item-height: 35px; /* 定义CSS变量 */
    padding-top: 10px;
    padding-bottom: 20px;
    border-bottom: 1px solid #f0f2f5;
}

.batch-rename-tools {
    margin-top: 8px;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 4px;
}

.batch-rename-item {
    display: flex;
    align-items: center;
    gap: 8px;
}

.batch-rename-input {
    width: 120px;
}

.schedule-item {
    gap: 6px;
}

.schedule-label {
    font-size: 12px;
    color: #606266;
    white-space: nowrap;
}

.schedule-date-picker {
    width: 135px;
}

.schedule-count-input {
    width: 70px;
}

.schedule-tip {
    font-size: 10px;
    color: #909399;
    white-space: nowrap;
}

.sort-tools {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
}

.uploaded-videos-section h4 {
    margin: 0 0 16px 0;
    font-size: 14px;
    font-weight: 500;
    color: #303133;
}

.uploaded-videos-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    scrollbar-width: thin;
    scrollbar-color: #c1c1c1 transparent;
    border-radius: 6px;
    border: 1px solid #e9ecef;
    padding: 8px;
    background: #fafbfc;
}

.uploaded-videos-list::-webkit-scrollbar {
    width: 6px;
}

.uploaded-videos-list::-webkit-scrollbar-track {
    background: transparent;
}

.uploaded-videos-list::-webkit-scrollbar-thumb {
    background-color: #c1c1c1;
    border-radius: 3px;
}

.uploaded-videos-list::-webkit-scrollbar-thumb:hover {
    background-color: #a8a8a8;
}

.uploaded-video-item {
    display: flex;
    align-items: center;
    padding: 4px 8px;
    background: #fff;
    border-radius: 4px;
    border: 1px solid #e9ecef;
    transition: all 0.3s;
    min-height: var(--video-item-height);
    flex-shrink: 0;
}

.uploaded-video-item:hover {
    background: #f0f9ff;
    border-color: #b3d8ff;
}

.video-order {
    margin-right: 12px;
    flex-shrink: 0;
}

.video-order .order-input {
    width: 70px;
}

.video-order :deep(.el-input-number .el-input__inner) {
    text-align: center;
    font-size: 12px;
    padding: 0 0px;
}

.video-order :deep(.el-input-number__increase),
.video-order :deep(.el-input-number__decrease) {
    width: 14px;
    font-size: 10px;
}

.video-status-icon {
    margin-right: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
}

.status-complete {
    color: #67c23a;
    font-size: 14px;
}

.status-uploading {
    color: #409eff;
    font-size: 12px;
    animation: rotate 1s linear infinite;
}

.status-pending {
    color: #909399;
    font-size: 12px;
}

.status-failed {
    color: #f56c6c;
    font-size: 14px;
}

.status-paused {
    color: #e6a23c;
    font-size: 14px;
}

.status-cancelled {
    color: #909399;
    font-size: 14px;
}

@keyframes rotate {
    from {
        transform: rotate(0deg);
    }
    to {
        transform: rotate(360deg);
    }
}

.video-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1px;
}

.video-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
}

.video-cover-row {
    margin-top: 8px;
}

.video-title-container {
    flex: 1;
    min-width: 0;
}

.video-title {
    font-size: 12px;
    font-weight: 500;
    color: #303133;
    line-height: 1.2;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 1px 3px;
    border-radius: 2px;
    transition: all 0.3s;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.video-title-text {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
}

.video-title:hover {
    background: #ecf5ff;
    color: #409eff;
}

.video-title:hover .edit-icon {
    opacity: 1;
}

.edit-icon {
    opacity: 0;
    font-size: 10px;
    flex-shrink: 0;
    transition: opacity 0.3s;
}

/* AI 标题生成 sparkle 图标 */
.ai-icon {
    flex-shrink: 0;
    width: 12px;
    height: 12px;
    color: #909399;
    opacity: 0.9;
    cursor: pointer;
    transition: all 0.25s ease;
}

.ai-icon:hover {
    color: #722ed1;
    opacity: 1;
    transform: scale(1.2);
    filter: drop-shadow(0 0 4px rgba(114, 46, 209, 0.55));
}

.ai-icon.is-generating {
    color: #722ed1;
    opacity: 1;
    cursor: wait;
    pointer-events: none;
    animation: ai-icon-spin 1.2s linear infinite;
}

@keyframes ai-icon-spin {
    from {
        transform: rotate(0deg);
    }
    to {
        transform: rotate(360deg);
    }
}

.video-title-edit {
    flex: 1;
    width: 100%;
    min-width: 0;
}

.video-title-edit :deep(.el-input) {
    width: 100%;
}

.video-status {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
}

.video-status .status-text {
    padding: 1px 4px;
    border-radius: 2px;
    font-size: 9px;
    font-weight: 500;
    line-height: 1.2;
}

.video-status .publish-time {
    padding: 1px 4px;
    border-radius: 2px;
    font-size: 9px;
    font-weight: 500;
    line-height: 1.2;
    white-space: nowrap;
    background: #f0f9eb;
    color: #67c23a;
}

.video-status .status-text.complete {
    background: #f0f9ff;
    color: #67c23a;
}

.video-status .status-text.uploading {
    background: #ecf5ff;
    color: #409eff;
}

.video-status .status-text.pending {
    background: #f4f4f5;
    color: #909399;
}

.video-status .status-text.failed {
    background: #fef0f0;
    color: #f56c6c;
}

.video-status .status-text.paused {
    background: #fdf6ec;
    color: #e6a23c;
}

.video-status .status-text.cancelled {
    background: #f4f4f5;
    color: #909399;
}

.progress-section {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-top: 1px;
}

.progress-bar-container {
    display: flex;
    align-items: center;
    gap: 4px;
}

.progress-bar-container :deep(.el-progress) {
    flex: 1;
    min-width: 60px;
}

.progress-text {
    font-size: 9px;
    font-weight: 500;
    color: #606266;
    min-width: 25px;
    text-align: right;
}

.upload-speed {
    font-size: 9px;
    color: #909399;
    text-align: right;
    font-family: 'Courier New', monospace;
    line-height: 1.2;
}

.error-message {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 9px;
    color: #f56c6c;
    background: #fef0f0;
    border: 1px solid #fbc4c4;
    border-radius: 3px;
    padding: 3px 6px;
    margin-top: 2px;
    line-height: 1.3;
    word-break: break-word;
    max-width: 100%;
}

.error-message .error-icon {
    font-size: 10px;
    color: #f56c6c;
    flex-shrink: 0;
}

.video-actions {
    margin-left: 6px;
    opacity: 1;
    display: flex;
    gap: 2px;
}

.video-buttons-group {
    display: flex;
    justify-content: center;
    gap: 3px;
    margin-bottom: 5px;
}

.upload-tip {
    font-size: 10px;
    color: #909399;
    text-align: center;
}

.drag-active-tip {
    color: #409eff;
    font-weight: 500;
}

/* 完成时间样式 */
.completed-time {
    font-size: 10px;
    color: #67c23a;
    font-weight: 500;
    margin-left: 8px;
}

/* 警告视频样式 */
.video-warning {
    border: 2px solid #e6a23c;
    border-radius: 6px;
    background: linear-gradient(to right, rgba(230, 162, 60, 0.05), rgba(230, 162, 60, 0.02));
    cursor: help;
    position: relative;
}

.video-warning::before {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 4px;
    background: linear-gradient(to bottom, #e6a23c, #f39c12);
    border-radius: 2px 0 0 2px;
}

.video-warning:hover {
    border-color: #f39c12;
    background: linear-gradient(to right, rgba(230, 162, 60, 0.1), rgba(230, 162, 60, 0.05));
    box-shadow: 0 2px 8px rgba(230, 162, 60, 0.3);
    transform: translateY(-1px);
    transition: all 0.3s ease;
}

.video-warning .completed-time {
    color: #e6a23c;
    font-weight: 600;
    animation: pulse-warning 2s infinite;
}

@keyframes pulse-warning {
    0%,
    100% {
        opacity: 1;
    }
    50% {
        opacity: 0.7;
    }
}

/* 多稿件提交模式下未就绪（未改名/未设置封面）的视频 */
.video-warning.video-rename-pending {
    border-color: #f56c6c;
    background: linear-gradient(to right, rgba(245, 108, 108, 0.06), rgba(245, 108, 108, 0.02));
}

.video-warning.video-rename-pending::before {
    background: linear-gradient(to bottom, #f56c6c, #e74c3c);
}

.video-warning.video-rename-pending:hover {
    border-color: #e74c3c;
    background: linear-gradient(to right, rgba(245, 108, 108, 0.12), rgba(245, 108, 108, 0.05));
    box-shadow: 0 2px 8px rgba(245, 108, 108, 0.3);
}

/* 超过10小时的视频使用更强烈的警告颜色 */
.video-warning.video-expired {
    border-color: #f56c6c;
    background: linear-gradient(to right, rgba(245, 108, 108, 0.05), rgba(245, 108, 108, 0.02));
}

.video-warning.video-expired::before {
    background: linear-gradient(to bottom, #f56c6c, #e74c3c);
}

.video-warning.video-expired:hover {
    border-color: #e74c3c;
    background: linear-gradient(to right, rgba(245, 108, 108, 0.1), rgba(245, 108, 108, 0.05));
    box-shadow: 0 2px 8px rgba(245, 108, 108, 0.3);
}

.video-warning.video-expired .completed-time {
    color: #f56c6c;
    animation: pulse-danger 1.5s infinite;
}

@keyframes pulse-danger {
    0%,
    100% {
        opacity: 1;
        transform: scale(1);
    }
    50% {
        opacity: 0.8;
        transform: scale(1.05);
    }
}

</style>
