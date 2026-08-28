<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { Clock } from '@element-plus/icons-vue'
import { useUtilsStore, type ArchiveListItem } from '../stores/utils'

const props = withDefaults(
    defineProps<{
        uid: number
        title: string
    }>(),
    {
        uid: 0,
        title: '',
    }
)

const utilsStore = useUtilsStore()

// 已发布稿件列表（用于展示模板对应视频的发布时间）
const publishedArchives = ref<ArchiveListItem[]>([])
const loading = ref(false)

// 已发布视频中标题含模板 title 的第一条数据（用于展示发布时间）
const lastPublishedInfo = computed(() => {
    const title = props.title
    if (!title || !publishedArchives.value.length) {
        return null
    }
    const match = publishedArchives.value.find((item) => item.title && item.title.includes(title))
    if (!match) {
        return null
    }
    // 有定时发布时间(dtime)优先展示定时发布时间，否则展示实际发布时间(ptime)
    const time = match.dtime > 0 ? match.dtime : match.ptime
    return {
        bvid: match.bvid,
        title: match.title,
        time,
        isScheduled: match.dtime > 0
    }
})

// 拉取当前用户的已发布稿件列表
const fetchPublishedArchives = async (uid: number) => {
    loading.value = true
    try {
        const title = props.title
        publishedArchives.value = await utilsStore
            .getArchives(uid, 'is_pubing,pubed', 1, 1, title)
            .then(archives => archives.filter(archive => archive.title.includes(title)))
    } catch (error) {
        console.error('获取已发布稿件失败:', error)
        publishedArchives.value = []
    } finally {
        loading.value = false
    }
}

// 用户或模板切换时自动重新拉取
watch(
    [() => props.uid, () => props.title],
    ([uid, title]) => {
        if (uid > 0 && title) {
            fetchPublishedArchives(uid)
        }
    },
    { immediate: true }
)

// 供父组件在投稿完成后手动刷新
const refresh = () => {
    if (props.uid > 0 && props.title) {
        fetchPublishedArchives(props.uid)
    }
}

defineExpose<{ refresh: () => void }>({ refresh })

// 格式化稿件时间戳（秒）为本地时间字符串
const formatPublishedTime = (timestamp: number): string => {
    if (!timestamp || timestamp <= 0) {
        return ''
    }
    return new Date(timestamp * 1000).toLocaleString()
}
</script>

<template>
    <span
        v-if="lastPublishedInfo"
        class="last-published-badge"
        :class="{ scheduled: lastPublishedInfo.isScheduled }"
        :title="`匹配稿件：${lastPublishedInfo.title}（${lastPublishedInfo.bvid}）`"
    >
        <el-icon><clock /></el-icon>
        {{ lastPublishedInfo.isScheduled ? '定时发布时间' : '上次发布时间' }}：
        {{ formatPublishedTime(lastPublishedInfo.time) }}
    </span>
    <span v-else-if="loading" class="last-published-badge loading">
        <el-icon><clock /></el-icon>
        查询已发布视频中…
    </span>
</template>

<style scoped>
.last-published-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 12px;
    line-height: 1.4;
    color: #606266;
    background: #f4f4f5;
    border: 1px solid #e4e7ed;
    white-space: nowrap;
}

.last-published-badge.scheduled {
    color: #e6a23c;
    background: #fdf6ec;
    border-color: #f3d19e;
}

.last-published-badge.loading {
    color: #909399;
}
</style>
