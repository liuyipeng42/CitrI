<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { useReasoning } from './useReasoning'
import { useAppEvents } from '@/lib/events'
import { renderMarkdown } from '@/lib/markdown'
import AppIcon from '@/components/AppIcon.vue'
import { command, errorMessage } from '@/lib/bridge'
import { applyAppearance } from '@/lib/theme'
import { coalescedTask } from '@/lib/coalescedTask'
import type { ActionSession, PublicSettings } from '@/types'
import './action.css'

const session = ref<ActionSession | null>(null)
const result = ref('')
const renderedResult = ref('')
let pendingResult = ''
let pendingReasoning = ''
const error = ref('')
const loading = ref(true)
const { reasoning, reasoningExpanded, reasoningActive, finishReasoning, resetReasoning } = useReasoning(loading)
const pinned = ref(false)
const opacity = ref(100)
const showOriginal = ref(false)
const focused = ref(true)
const footerVisible = ref(true)
const copied = ref<'original' | 'result' | ''>('')
const scroll = ref<HTMLElement | null>(null)
const showOpacityPopup = ref(false)
const autoScroll = ref(true)
const listenEvent = useAppEvents()
const resizeDirections = [
  'north', 'north-east', 'east', 'south-east',
  'south', 'south-west', 'west', 'north-west',
] as const
type ResizeDirection = (typeof resizeDirections)[number]
let footerTimer: number | undefined
let copyTimer: number | undefined
let footerHideAt = 0

// Markdown parsing and sanitizing process the entire answer. Limit streaming
// renders to 20/s; completion flushes the last update without a delay.
const resultRenderer = coalescedTask(() => {
  if (pendingReasoning) { reasoning.value += pendingReasoning; pendingReasoning = '' }
  if (!pendingResult) return
  result.value += pendingResult
  pendingResult = ''
  renderedResult.value = renderMarkdown(result.value)
  void nextTick(() => {
    if (autoScroll.value && scroll.value) scroll.value.scrollTo({ top: scroll.value.scrollHeight })
  })
}, 50)

function start(): void {
  resultRenderer.cancel()
  pendingResult = ''
  pendingReasoning = ''
  result.value = ''
  resetReasoning()
  renderedResult.value = ''
  error.value = ''
  loading.value = true
  autoScroll.value = true
  void command<void>('assistant_start').catch((reason) => {
    finishReasoning()
    loading.value = false
    error.value = errorMessage(reason)
  })
}

async function stopOrClose(): Promise<void> {
  if (loading.value) await command('assistant_stop')
  else await command('window_control', { operation: 'close' })
}

async function regenerate(): Promise<void> {
  if (loading.value) await command('assistant_stop')
  start()
}

async function copyText(text: string, type: 'original' | 'result'): Promise<void> {
  if (!text) return
  await command('clipboard_write', { text })
  copied.value = type
  window.clearTimeout(copyTimer)
  copyTimer = window.setTimeout(() => (copied.value = ''), 1000)
}

async function togglePin(): Promise<void> {
  pinned.value = !pinned.value
  pinned.value = await command<boolean>('window_control', { operation: 'pin', value: pinned.value })
}

function hideFooterWhenIdle(): void {
  const remaining = footerHideAt - performance.now()
  if (remaining > 0) {
    footerTimer = window.setTimeout(hideFooterWhenIdle, remaining)
    return
  }
  footerTimer = undefined
  footerVisible.value = false
}

function revealFooter(): void {
  footerVisible.value = true
  footerHideAt = performance.now() + 2500
  if (footerTimer === undefined) footerTimer = window.setTimeout(hideFooterWhenIdle, 2500)
}

function onScroll(): void {
  if (!scroll.value) return
  autoScroll.value = Math.abs(scroll.value.scrollHeight - scroll.value.scrollTop - scroll.value.clientHeight) < 32
}

function openResultLink(event: MouseEvent): void {
  const anchor = (event.target as HTMLElement).closest<HTMLAnchorElement>('a[href]')
  if (!anchor) return
  event.preventDefault()
  void command('open_target', { target: anchor.href })
}

function onKeydown(event: KeyboardEvent): void {
  if (event.ctrlKey || event.metaKey || event.altKey) return
  const key = event.key.toLowerCase()
  if (key === 'escape') void stopOrClose()
  else if (key === 'r') void regenerate()
  else if (key === 'c' && !loading.value) void copyText(result.value, 'result')
  revealFooter()
}

function startWindowDrag(): void {
  void command('window_control', { operation: 'drag' })
}

function startWindowResize(direction: ResizeDirection): void {
  void command('window_control', { operation: 'resize', direction })
}

function onFocus(): void {
  focused.value = true
}

function onBlur(): void {
  focused.value = false
}

onMounted(async () => {
  if (!await listenEvent<string>('assistant-reasoning', (event) => {
    if (!loading.value) return
    pendingReasoning += event.payload ?? ''
    resultRenderer.schedule()
  })) return
  if (!await listenEvent<string>('assistant-delta', (event) => {
    if (!loading.value) return
    if (event.payload) finishReasoning()
    pendingResult += event.payload ?? ''
    resultRenderer.schedule()
  })) return
  if (!await listenEvent<null>('assistant-done', () => { finishReasoning(); loading.value = false; resultRenderer.flush() })) return
  if (!await listenEvent<string>('assistant-error', (event) => { finishReasoning(); loading.value = false; error.value = event.payload; resultRenderer.flush() })) return
  if (!await listenEvent<PublicSettings>('settings-changed', (event) => {
    opacity.value = event.payload.actionOpacity
    applyAppearance(event.payload)
  })) return
  try {
    session.value = await command<ActionSession>('get_action_session')
    document.title = `${session.value.action.name} - CitrI`
    applyAppearance(session.value.settings)
    pinned.value = session.value.settings.autoPin
    opacity.value = session.value.settings.actionOpacity
    await nextTick()
    await command('window_control', { operation: 'ready' })
    start()
  } catch (reason) {
    error.value = errorMessage(reason)
    loading.value = false
  }
  window.addEventListener('keydown', onKeydown)
  window.addEventListener('focus', onFocus)
  window.addEventListener('blur', onBlur)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  window.removeEventListener('focus', onFocus)
  window.removeEventListener('blur', onBlur)
  resultRenderer.cancel()
  window.clearTimeout(footerTimer)
  window.clearTimeout(copyTimer)
})
</script>

<template>
  <main class="action-window" :style="{ opacity: opacity / 100 }" @mousemove="revealFooter" @mousedown="showOpacityPopup = false">
    <div v-for="direction in resizeDirections" :key="direction" :class="['resize-handle', `resize-${direction}`]" @mousedown.left.prevent.stop="startWindowResize(direction)" />
    <header class="action-titlebar" :class="{ focused }" @mousedown.left="startWindowDrag">
      <span class="title-icon"><AppIcon :name="session?.action.icon ?? 'sparkles'" :size="14" /></span>
      <div class="title">{{ session?.action.name ?? 'CitrI' }}</div>
      <div class="window-actions" @mousedown.stop>
        <button type="button" title="置顶" :class="{ active: pinned }" @click="togglePin"><AppIcon name="pin" :size="13" /></button>
        <div class="opacity-wrap">
          <button type="button" title="透明度" @click="showOpacityPopup = !showOpacityPopup"><AppIcon name="droplet" :size="13" /></button>
          <div class="opacity-popup" :class="{ hidden: !showOpacityPopup }"><span>{{ opacity }}%</span><input v-model.number="opacity" type="range" min="20" max="100" @change="showOpacityPopup = false" /></div>
        </div>
        <button type="button" title="最小化" @click="command('window_control', { operation: 'minimize' })"><AppIcon name="minus" :size="13" /></button>
        <button class="close" type="button" title="关闭" @click="command('window_control', { operation: 'close' })"><AppIcon name="x" :size="13" /></button>
      </div>
    </header>

    <section ref="scroll" class="content-scroll" @scroll="onScroll">
      <div v-if="session?.action.id !== 'translate'" class="original-bar">
        <button class="original-toggle" :class="{ open: showOriginal }" type="button" @click="showOriginal = !showOriginal"><span>{{ showOriginal ? '隐藏原文' : '显示原文' }}</span><AppIcon name="chevron" :size="13" /></button>
      </div>
      <div v-if="showOriginal" class="original"><div>{{ session?.selectedText }}</div><button type="button" title="复制原文" @click="copyText(session?.selectedText ?? '', 'original')"><AppIcon :name="copied === 'original' ? 'check' : 'copy'" :size="13" /></button></div>
      <div v-if="loading" class="generating"><span class="spinner" /><span>{{ result ? '正在生成回答…' : reasoning ? '正在思考…' : '正在等待模型响应…' }}</span></div>
      <section v-if="reasoning" class="reasoning-panel" :class="{ expanded: reasoningExpanded, thinking: reasoningActive }">
        <button class="reasoning-toggle" type="button" :aria-expanded="reasoningExpanded" aria-controls="reasoning-content" @click="reasoningExpanded = !reasoningExpanded">
          <span class="reasoning-label"><span class="reasoning-indicator" />{{ reasoningActive ? '正在思考' : '思考记录' }}</span>
          <span class="reasoning-disclosure">{{ reasoningExpanded ? '收起' : '查看思考' }}<AppIcon name="chevron" :size="13" /></span>
        </button>
        <div id="reasoning-content" class="reasoning-body" :aria-hidden="!reasoningExpanded" :inert="!reasoningExpanded">
          <div class="reasoning-clip">
            <div ref="reasoningScroll" class="reasoning-content" tabindex="0" aria-label="思考内容">{{ reasoning }}</div>
          </div>
        </div>
      </section>
      <article v-if="result" class="result markdown" v-html="renderedResult" @click="openResultLink" />
      <div v-if="error" class="result-error">{{ error }}</div>
      <div class="footer-spacer" />
    </section>

    <footer class="action-footer" :class="{ visible: footerVisible }" @mouseenter="revealFooter">
      <button class="danger" type="button" @click="stopOrClose"><AppIcon :name="loading ? 'pause' : 'circlex'" :size="13" /><kbd>Esc</kbd><span>{{ loading ? '停止' : '关闭' }}</span></button>
      <button type="button" @click="regenerate"><AppIcon name="refresh" :size="13" /><kbd>R</kbd><span>重新生成</span></button>
      <button type="button" :disabled="loading || !result" @click="copyText(result, 'result')"><AppIcon :name="copied === 'result' ? 'check' : 'copy'" :size="13" /><kbd v-if="copied !== 'result'">C</kbd><span>{{ copied === 'result' ? '已复制' : '复制' }}</span></button>
    </footer>
  </main>
</template>
