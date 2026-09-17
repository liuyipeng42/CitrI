<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import DOMPurify from 'dompurify'
import { marked } from 'marked'
import AppIcon from '@/components/AppIcon.vue'
import { command, errorMessage, onEvent } from '@/lib/bridge'
import { applyAppearance } from '@/lib/theme'
import type { ActionSession, PublicSettings } from '@/types'
import './action.css'

marked.setOptions({ gfm: true, breaks: true })

const session = ref<ActionSession | null>(null)
const result = ref('')
const reasoning = ref('')
const reasoningExpanded = ref(true)
const reasoningFinished = ref(false)
const reasoningScroll = ref<HTMLElement | null>(null)
const reasoningActive = computed(() => loading.value && !reasoningFinished.value)
const renderedResult = ref('')
const error = ref('')
const loading = ref(true)
const pinned = ref(false)
const opacity = ref(100)
const showOriginal = ref(false)
const focused = ref(true)
const footerVisible = ref(true)
const copied = ref<'original' | 'result' | ''>('')
const scroll = ref<HTMLElement | null>(null)
const opacityPopup = ref<HTMLElement | null>(null)
const autoScroll = ref(true)
const unlisteners: UnlistenFn[] = []
const resizeDirections = [
  'north', 'north-east', 'east', 'south-east',
  'south', 'south-west', 'west', 'north-west',
] as const
type ResizeDirection = (typeof resizeDirections)[number]
let footerTimer: number | undefined
let copyTimer: number | undefined
let renderFrame: number | undefined
let footerHideAt = 0
let reasoningFrame: number | undefined

function finishReasoning(): void {
  if (reasoningFinished.value) return
  reasoningFinished.value = true
  reasoningExpanded.value = false
  if (reasoningFrame !== undefined) cancelAnimationFrame(reasoningFrame)
  reasoningFrame = undefined
}

function followReasoning(): void {
  if (reasoningFrame !== undefined || !reasoningActive.value || !reasoningExpanded.value) return
  let previous = performance.now()
  const step = (now: number): void => {
    reasoningFrame = undefined
    const viewport = reasoningScroll.value
    if (!viewport || !reasoningActive.value || !reasoningExpanded.value) return
    const target = Math.max(0, viewport.scrollHeight - viewport.clientHeight)
    const distance = target - viewport.scrollTop
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    if (Math.abs(distance) < 1 || reducedMotion) {
      viewport.scrollTop = target
      return
    }
    const movement = Math.max(1, Math.abs(distance) * (1 - Math.exp(-(now - previous) / 65)))
    viewport.scrollTop += Math.sign(distance) * Math.min(Math.abs(distance), movement)
    previous = now
    reasoningFrame = requestAnimationFrame(step)
  }
  reasoningFrame = requestAnimationFrame(step)
}

watch([reasoning, reasoningExpanded], followReasoning, { flush: 'post' })

function queueRender(): void {
  if (renderFrame !== undefined) return
  renderFrame = requestAnimationFrame(() => {
    renderFrame = undefined
    renderedResult.value = DOMPurify.sanitize(marked.parse(result.value, { async: false }) as string, {
      USE_PROFILES: { html: true },
      ADD_ATTR: ['target', 'rel'],
    })
    void nextTick(() => {
      if (autoScroll.value && scroll.value) scroll.value.scrollTo({ top: scroll.value.scrollHeight })
    })
  })
}

function start(): void {
  if (renderFrame !== undefined) cancelAnimationFrame(renderFrame)
  renderFrame = undefined
  result.value = ''
  reasoning.value = ''
  reasoningExpanded.value = true
  reasoningFinished.value = false
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

function toggleOpacityPopup(): void {
  opacityPopup.value?.classList.toggle('hidden')
}

function hideOpacityPopup(): void {
  opacityPopup.value?.classList.add('hidden')
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
  unlisteners.push(await onEvent<string>('assistant-reasoning', (event) => {
    reasoning.value += event.payload ?? ''
  }))
  unlisteners.push(await onEvent<string>('assistant-delta', (event) => {
    if (event.payload) finishReasoning()
    result.value += event.payload ?? ''
    loading.value = true
    queueRender()
  }))
  unlisteners.push(await onEvent<null>('assistant-done', () => { finishReasoning(); loading.value = false; queueRender() }))
  unlisteners.push(await onEvent<string>('assistant-error', (event) => { finishReasoning(); loading.value = false; error.value = event.payload }))
  unlisteners.push(await onEvent<PublicSettings>('settings-changed', (event) => applyAppearance(event.payload)))
  try {
    session.value = await command<ActionSession>('get_action_session')
    document.title = `${session.value.action.name} - CitrI`
    applyAppearance(session.value.settings)
    pinned.value = session.value.settings.autoPin
    opacity.value = session.value.settings.actionOpacity
    await command('window_control', { operation: 'pin', value: pinned.value })
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
  unlisteners.forEach((unlisten) => unlisten())
  window.removeEventListener('keydown', onKeydown)
  window.removeEventListener('focus', onFocus)
  window.removeEventListener('blur', onBlur)
  if (renderFrame !== undefined) cancelAnimationFrame(renderFrame)
  if (reasoningFrame !== undefined) cancelAnimationFrame(reasoningFrame)
  window.clearTimeout(footerTimer)
  window.clearTimeout(copyTimer)
})
</script>

<template>
  <main class="action-window" :style="{ opacity: opacity / 100 }" @mousemove="revealFooter">
    <div v-for="direction in resizeDirections" :key="direction" :class="['resize-handle', `resize-${direction}`]" @mousedown.left.prevent.stop="startWindowResize(direction)" />
    <header class="action-titlebar" :class="{ focused }" @mousedown.left="startWindowDrag">
      <span class="title-icon"><AppIcon :name="session?.action.icon ?? 'sparkles'" :size="14" /></span>
      <div class="title">{{ session?.action.name ?? 'CitrI' }}</div>
      <div class="window-actions" @mousedown.stop>
        <button type="button" title="置顶" :class="{ active: pinned }" @click="togglePin"><AppIcon name="pin" :size="13" /></button>
        <div class="opacity-wrap">
          <button type="button" title="透明度" @click="toggleOpacityPopup"><AppIcon name="droplet" :size="13" /></button>
          <div ref="opacityPopup" class="opacity-popup hidden"><span>{{ opacity }}%</span><input v-model.number="opacity" type="range" min="20" max="100" @change="hideOpacityPopup" /></div>
        </div>
        <button type="button" title="最小化" @click="command('window_control', { operation: 'minimize' })"><AppIcon name="minus" :size="13" /></button>
        <button class="close" type="button" title="关闭" @click="command('window_control', { operation: 'close' })"><AppIcon name="x" :size="13" /></button>
      </div>
    </header>

    <section ref="scroll" class="content-scroll" @scroll="onScroll">
      <button v-if="session?.action.id !== 'translate'" class="original-toggle" :class="{ open: showOriginal }" type="button" @click="showOriginal = !showOriginal"><span>{{ showOriginal ? '隐藏原文' : '显示原文' }}</span><AppIcon name="chevron" :size="14" /></button>
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
      <button class="danger" type="button" @click="stopOrClose"><AppIcon :name="loading ? 'pause' : 'circlex'" :size="14" /><span>Esc {{ loading ? '停止' : '关闭' }}</span></button>
      <button type="button" @click="regenerate"><AppIcon name="refresh" :size="14" /><span>R 重新生成</span></button>
      <button type="button" :disabled="loading || !result" @click="copyText(result, 'result')"><AppIcon :name="copied === 'result' ? 'check' : 'copy'" :size="14" /><span>{{ copied === 'result' ? '已复制' : 'C 复制' }}</span></button>
    </footer>
  </main>
</template>
