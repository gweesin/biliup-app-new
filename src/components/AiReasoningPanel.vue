<template>
    <div v-if="reasoning || thinking" class="ai-reasoning" :class="{ 'is-collapsed': !expanded }">
        <!-- 头部：点击展开/收起 -->
        <div class="ai-reasoning-head" role="button" :aria-expanded="expanded" @click="toggle">
            <span class="ai-reasoning-icon" :class="{ 'is-thinking': thinking }">
                <el-icon><magic-stick /></el-icon>
            </span>
            <span class="ai-reasoning-label">深度思考</span>
            <span class="ai-reasoning-hint">
                <template v-if="thinking">
                    <span class="thinking-text">思考中</span>
                    <span class="thinking-dots"><i /><i /><i /></span>
                </template>
                <template v-else-if="reasoning">
                    {{ charCount }} 字 · {{ expanded ? '收起' : '展开' }}
                </template>
            </span>
            <span class="ai-reasoning-caret">
                <el-icon><arrow-down /></el-icon>
            </span>
        </div>

        <!-- 思考过程正文（可滚动） -->
        <el-collapse-transition>
            <div v-show="expanded" class="ai-reasoning-body-wrap">
                <div ref="bodyEl" class="ai-reasoning-body">
                    <template v-if="reasoning">
                        <span class="ai-reason-text">{{ reasoning }}</span>
                        <span v-if="thinking" class="ai-caret" />
                    </template>
                    <span v-else class="ai-reason-placeholder">正在分析画面与对局信息…</span>
                </div>
            </div>
        </el-collapse-transition>
    </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { ArrowDown, MagicStick } from '@element-plus/icons-vue'

interface Props {
    /** 已累积的思考过程文本 */
    reasoning: string
    /** 是否正在思考（流式进行中） */
    thinking: boolean
}

const props = defineProps<Props>()

const expanded = ref(false)
const bodyEl = ref<HTMLElement | null>(null)
let collapseTimer: ReturnType<typeof setTimeout> | null = null

const charCount = computed(() => props.reasoning.trim().length)

// 思考开始时自动展开；结束后稍作停留自动收起（仍可点头部回看）
watch(
    () => props.thinking,
    thinking => {
        if (collapseTimer) {
            clearTimeout(collapseTimer)
            collapseTimer = null
        }
        if (thinking) {
            expanded.value = true
        } else {
            collapseTimer = setTimeout(() => {
                expanded.value = false
            }, 600)
        }
    },
    { immediate: true }
)

// 内容持续追加时跟随滚动到底部
watch([() => props.reasoning, expanded], () => {
    if (expanded.value) {
        nextTick(() => {
            if (bodyEl.value) {
                bodyEl.value.scrollTop = bodyEl.value.scrollHeight
            }
        })
    }
})

const toggle = () => {
    // 用户手动交互时取消自动收起
    if (collapseTimer) {
        clearTimeout(collapseTimer)
        collapseTimer = null
    }
    expanded.value = !expanded.value
}

onBeforeUnmount(() => {
    if (collapseTimer) {
        clearTimeout(collapseTimer)
    }
})
</script>

<style scoped>
.ai-reasoning {
    margin: 5px 0 2px;
    border: 1px solid #e7e3f3;
    border-radius: 8px;
    background: #faf9fd;
    overflow: hidden;
    transition: border-color 0.2s ease, box-shadow 0.2s ease;
}

.ai-reasoning.is-thinking {
    border-color: #d9d0f0;
    box-shadow: 0 0 0 1px rgba(114, 46, 209, 0.06);
}

.ai-reasoning-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    cursor: pointer;
    user-select: none;
    transition: background 0.2s ease;
}

.ai-reasoning-head:hover {
    background: #f3f0fb;
}

.ai-reasoning-icon {
    display: inline-flex;
    align-items: center;
    color: #722ed1;
    font-size: 13px;
    line-height: 1;
    flex-shrink: 0;
}

.ai-reasoning-icon.is-thinking {
    animation: ai-think-spin 2.4s linear infinite;
}

@keyframes ai-think-spin {
    from {
        transform: rotate(0deg);
    }
    to {
        transform: rotate(360deg);
    }
}

.ai-reasoning-label {
    font-size: 11px;
    font-weight: 600;
    color: #4a4172;
    flex-shrink: 0;
}

.ai-reasoning-hint {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    color: #9a92bd;
    margin-left: auto;
    flex-shrink: 0;
}

.thinking-dots {
    display: inline-flex;
    gap: 2px;
}

.thinking-dots i {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: #8b7fd0;
    animation: ai-dot-blink 1.2s ease-in-out infinite;
}

.thinking-dots i:nth-child(2) {
    animation-delay: 0.2s;
}

.thinking-dots i:nth-child(3) {
    animation-delay: 0.4s;
}

@keyframes ai-dot-blink {
    0%,
    60%,
    100% {
        opacity: 0.25;
        transform: translateY(0);
    }
    30% {
        opacity: 1;
        transform: translateY(-1px);
    }
}

.ai-reasoning-caret {
    display: inline-flex;
    align-items: center;
    color: #b3abcf;
    font-size: 11px;
    flex-shrink: 0;
    transition: transform 0.25s ease;
}

.ai-reasoning.is-collapsed .ai-reasoning-caret {
    transform: rotate(-90deg);
}

.ai-reasoning-body-wrap {
    border-top: 1px dashed #e7e3f3;
}

.ai-reasoning-body {
    max-height: 160px;
    overflow-y: auto;
    padding: 6px 10px 8px;
    font-size: 11px;
    line-height: 1.65;
    color: #5a5580;
    white-space: pre-wrap;
    word-break: break-word;
    scrollbar-width: thin;
    scrollbar-color: #d5cdec transparent;
}

.ai-reasoning-body::-webkit-scrollbar {
    width: 5px;
}

.ai-reasoning-body::-webkit-scrollbar-track {
    background: transparent;
}

.ai-reasoning-body::-webkit-scrollbar-thumb {
    background-color: #d5cdec;
    border-radius: 3px;
}

.ai-reason-text {
    white-space: pre-wrap;
}

/* 思考中的闪烁光标 */
.ai-caret {
    display: inline-block;
    width: 1.5px;
    height: 12px;
    background: #722ed1;
    margin-left: 2px;
    vertical-align: -2px;
    animation: ai-caret-blink 0.85s step-end infinite;
}

@keyframes ai-caret-blink {
    0%,
    100% {
        opacity: 1;
    }
    50% {
        opacity: 0;
    }
}

.ai-reason-placeholder {
    color: #a9a1c9;
    font-style: italic;
}
</style>
