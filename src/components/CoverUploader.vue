<template>
    <div class="cover-uploader-row">
        <div
            class="cover-uploader"
            action="#"
            @click="handleCoverSelection"
            v-loading="coverLoading"
            :class="{ disabled: disabled }"
        >
            <img
                v-if="coverDisplayUrl && !coverLoading"
                :src="coverDisplayUrl"
                class="cover-image"
            />
            <el-icon v-else-if="!coverLoading" class="cover-uploader-icon">
                <plus />
            </el-icon>
        </div>

        <el-button
            v-if="coverDisplayUrl && !coverLoading"
            class="cover-clear-btn-side"
            type="danger"
            size="small"
            @click.stop="clearCurrentCover"
            :disabled="disabled"
            title="清除封面"
        >
            <el-icon><Close /></el-icon>
        </el-button>
    </div>
    <!-- 标题关键字匹配封面 -->
    <div v-if="coverMatchImages.length > 0" class="cover-match-list">
        <div class="cover-match-label">
            匹配到
            {{
                coverMatchImages.length
            }}
            张封面，点击即可设置：
        </div>
        <div class="cover-match-grid" v-loading="coverMatchLoading">
            <div
                v-for="img in coverMatchImages"
                :key="img.path"
                class="cover-match-item"
                :class="{
                    active: selectedMatchPath === img.path
                }"
                :title="img.name"
                @click="setCoverFromMatch(img.path)"
            >
                <img :src="img.data_url" :alt="img.name" loading="lazy" />
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import COVER_MATCH_KEYWORDS from '../constants/cover-match-keywords.json'
import { useUtilsStore } from '../stores/utils'
import { useUserConfigStore } from '../stores/user_config'

const props = defineProps<{
    modelValue: string
    title?: string
    uid?: number
    disabled?: boolean
}>()

const emit = defineEmits<{
    (e: 'update:modelValue', value: string): void
}>()

const utilsStore = useUtilsStore()
const userConfigStore = useUserConfigStore()

// 封面显示URL
const coverDisplayUrl = ref<string>('')
const coverLoading = ref<boolean>(false)

interface CoverMatchImage {
    name: string
    path: string
    data_url: string
}

const coverMatchImages = ref<CoverMatchImage[]>([])
const coverMatchLoading = ref<boolean>(false)
const selectedMatchPath = ref<string>('')
let coverMatchTimer: ReturnType<typeof setTimeout> | null = null
// 上次匹配的关键字组合（含匹配路径），用于避免相同关键字重复请求
let lastMatchKey = ''

// 监听封面变化，下载封面
watch(
    () => props.modelValue,
    async (newCover: string | undefined) => {
        if (newCover && props.uid) {
            try {
                coverLoading.value = true
                const downloadedCover = await utilsStore.downloadCover(props.uid, newCover)
                coverDisplayUrl.value = downloadedCover || ''
            } catch (error) {
                console.error('Failed to download cover:', error)
                clearCurrentCover()
            } finally {
                coverLoading.value = false
            }
        } else {
            coverDisplayUrl.value = ''
            coverLoading.value = false
        }
    },
    { immediate: true }
)

// 根据标题匹配封面路径中的图片
const refreshCoverMatch = async (title: string) => {
    // 清除旧定时器
    if (coverMatchTimer) {
        clearTimeout(coverMatchTimer)
        coverMatchTimer = null
    }

    // 没有配置封面匹配路径时清空
    const coverMatchPath = userConfigStore.configRoot?.cover_match_path || ''
    if (!title || !coverMatchPath) {
        coverMatchImages.value = []
        lastMatchKey = ''
        return
    }

    // 提取标题中包含的关键字
    const matchedKeywords = COVER_MATCH_KEYWORDS.filter(keyword => title.includes(keyword))
    if (matchedKeywords.length === 0) {
        coverMatchImages.value = []
        lastMatchKey = ''
        return
    }

    // 上次命中的关键字组合一致时无需重新请求
    const matchKey = `${coverMatchPath}|${matchedKeywords.join(',')}`
    if (matchKey === lastMatchKey) {
        return
    }

    // 防抖，避免输入过程中频繁调用
    coverMatchTimer = setTimeout(async () => {
        lastMatchKey = matchKey
        coverMatchLoading.value = true
        try {
            const images = await invoke<CoverMatchImage[]>('list_cover_images', {
                dirPath: coverMatchPath,
                keywords: matchedKeywords
            })
            coverMatchImages.value = images || []
        } catch (error) {
            console.error('匹配封面失败:', error)
            coverMatchImages.value = []
        } finally {
            coverMatchLoading.value = false
        }
    }, 300)
}

// 点击匹配的封面图片，上传并设置为封面
const setCoverFromMatch = async (filePath: string) => {
    if (props.disabled) {
        return
    }
    if (!props.uid) {
        utilsStore.showMessage('请先选择用户和模板', 'warning')
        return
    }

    try {
        coverLoading.value = true
        const url = await utilsStore.uploadCover(props.uid, filePath)
        if (url) {
            selectedMatchPath.value = filePath
            emit('update:modelValue', url)
            utilsStore.showMessage('已设置封面', 'success')
        } else {
            throw new Error('封面上传失败')
        }
    } catch (error) {
        console.error('设置封面失败: ', error)
        utilsStore.showMessage(`设置封面失败: ${error}`, 'error')
    } finally {
        coverLoading.value = false
    }
}

// 监听标题变化，实时匹配封面（immediate 确保组件挂载时已有标题也能立即匹配）
watch(
    () => props.title,
    (newTitle: string | undefined) => {
        refreshCoverMatch(newTitle || '')
    },
    { immediate: true }
)

// 监听用户切换，重新加载封面
watch(
    () => props.uid,
    async (newUid: number | undefined) => {
        if (props.modelValue && newUid) {
            try {
                coverLoading.value = true
                const downloadedCover = await utilsStore.downloadCover(
                    newUid,
                    props.modelValue
                )
                coverDisplayUrl.value = downloadedCover || ''
            } catch (error) {
                console.error('Failed to download cover:', error)
                clearCurrentCover()
            } finally {
                coverLoading.value = false
            }
        } else {
            coverDisplayUrl.value = ''
            coverLoading.value = false
        }
    }
)

// 选择封面
const handleCoverSelection = () => {
    if (!props.disabled && !coverLoading.value) {
        selectCoverWithTauri()
    }
}

const selectCoverWithTauri = async () => {
    try {
        const selected = await open({
            multiple: false,
            filters: [
                {
                    name: 'Image',
                    extensions: ['jpg', 'jpeg', 'png', 'pjp', 'pjpeg', 'jiff', 'gif']
                }
            ]
        })

        if (!selected || selected.length === 0) {
            utilsStore.showMessage('未选择任何封面文件', 'warning')
            return
        }

        if (props.uid) {
            coverLoading.value = true
            const url = await utilsStore.uploadCover(props.uid, selected)
            if (url) {
                emit('update:modelValue', url)
            } else {
                throw new Error('封面上传失败')
            }
        } else {
            utilsStore.showMessage('请先选择用户和模板', 'error')
        }
    } catch (error) {
        console.error('封面选择失败: ', error)
        utilsStore.showMessage(`'封面选择失败: ${error}'`, 'error')
        return
    } finally {
        coverLoading.value = false
    }
}

const clearCurrentCover = () => {
    if (props.disabled) {
        return
    }

    if (!props.uid) {
        utilsStore.showMessage('请先选择用户和模板', 'warning')
        return
    }

    emit('update:modelValue', '')
    coverDisplayUrl.value = ''
    utilsStore.showMessage('已清除封面', 'success')
}

// 组件卸载时清理定时器
onBeforeUnmount(() => {
    if (coverMatchTimer) {
        clearTimeout(coverMatchTimer)
        coverMatchTimer = null
    }
})
</script>

<style scoped>
.cover-uploader-row {
    display: inline-flex;
    align-items: center;
    gap: 10px;
}

.cover-match-list {
    margin-top: 12px;
    width: 100%;
}

.cover-match-label {
    font-size: 12px;
    color: #8c939d;
    margin-bottom: 8px;
}

.cover-match-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    min-height: 40px;
}

.cover-match-item {
    width: 84px;
    height: 48px;
    border-radius: 4px;
    overflow: hidden;
    cursor: pointer;
    border: 2px solid transparent;
    position: relative; /* 让 z-index 生效 */
    z-index: 1;
    transition:
        border-color 0.2s ease,
        transform 0.2s ease,
        box-shadow 0.2s ease;
}

.cover-match-item img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
}

.cover-match-item:hover {
    transform: scale(4) translateX(25px);
    border-color: #409eff;
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.2);
    z-index: 999; /* 确保悬浮时在最顶层 */
}

.cover-match-item.active {
    border-color: #409eff;
    box-shadow: 0 0 0 2px rgba(64, 158, 255, 0.3);
}

.cover-uploader {
    position: relative;
    display: inline-block;
    z-index: 1; /* 确保容器有基础层级 */
}

.cover-uploader .cover-image {
    width: 100px;
    height: 60px;
    object-fit: cover;
    border-radius: 4px;
    transition:
        transform 0.3s ease,
        box-shadow 0.3s ease;
    cursor: pointer;
    position: relative; /* 重要：让 z-index 生效 */
}

.cover-clear-btn-side {
    align-self: center;
    background: transparent;
    border: none;
    box-shadow: none;
    color: #9ca3af;
    padding: 0;
    min-width: 14px;
    width: 14px;
    height: 14px;
    line-height: 14px;
    font-size: 12px;
    transition: opacity 0.2s ease;
}

.cover-clear-btn-side:hover {
    background: transparent;
    border: none;
    color: #ef4444;
}

.cover-clear-btn-side :deep(.el-icon) {
    font-size: 12px;
}

.cover-uploader:hover + .cover-clear-btn-side {
    opacity: 0;
    pointer-events: none;
}

.cover-uploader .cover-image:hover {
    transform: scale(4) translateX(25px);
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.2);
    z-index: 999; /* 确保悬浮时在最顶层 */
    position: relative; /* 确保定位生效 */
}

.cover-uploader-icon {
    width: 100px;
    height: 60px;
    border: 1px dashed #d9d9d9;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #8c939d;
    font-size: 24px;
}

/* 禁用状态样式 */
.cover-uploader.disabled {
    cursor: not-allowed !important;
    opacity: 0.6 !important;
}

.cover-uploader.disabled:hover {
    border-color: #dcdfe6 !important;
}
</style>
