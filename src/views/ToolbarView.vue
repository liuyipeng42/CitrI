<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import AppIcon from '@/components/AppIcon.vue'
import { command, errorMessage, onEvent } from '@/lib/bridge'
import { applyAppearance } from '@/lib/theme'
import type { PublicSettings, SelectionPayload, ToolbarState } from '@/types'
import logoUrl from '@/assets/logo.png'
import './toolbar.css'

const toolbar = ref<HTMLElement | null>(null)
const selectedText = ref('')
const copiedAction = ref('')
const busyAction = ref('')
const actionError = ref('')
const settings = ref<PublicSettings>({
  compact: false, autoClose: true, autoPin: false, actionOpacity: 100,
  fontSize: 100, accentColor: 'orange', customAccentColor: '#f97316', actions: [],
})
const unlisteners: UnlistenFn[] = []
let resizeObserver: ResizeObserver | undefined
let copyTimer: number | undefined
let errorTimer: number | undefined
let resizePending = false
let lastWidth = 0
let lastHeight = 0

const enabledActions = computed(() => settings.value.actions.filter((action) => action.enabled))

function reportSize(): void {
  if (resizePending) return
  resizePending = true
  void nextTick(() => {
    resizePending = false
    if (!toolbar.value) return
    const rect = toolbar.value.getBoundingClientRect()
    const width = Math.ceil(rect.width + 6)
    const height = Math.ceil(rect.height + 7)
    if (width === lastWidth && height === lastHeight) return
    lastWidth = width
    lastHeight = height
    void command('toolbar_resize', { width, height })
  })
}

function applySettings(next: PublicSettings): void {
  settings.value = { ...settings.value, ...next }
  applyAppearance(settings.value)
  reportSize()
}

async function perform(actionId: string): Promise<void> {
  if (!selectedText.value || busyAction.value) return
  busyAction.value = actionId
  actionError.value = ''
  try {
    const completed = await command<boolean>('perform_action', { actionId, selectedText: selectedText.value })
    if (completed && actionId === 'copy') {
      copiedAction.value = actionId
      window.clearTimeout(copyTimer)
      copyTimer = window.setTimeout(() => (copiedAction.value = ''), 1100)
    }
  } catch (reason) {
    actionError.value = errorMessage(reason)
    window.clearTimeout(errorTimer)
    errorTimer = window.setTimeout(() => {
      actionError.value = ''
      reportSize()
    }, 4000)
  } finally {
    busyAction.value = ''
    reportSize()
  }
}

onMounted(async () => {
  unlisteners.push(await onEvent<SelectionPayload>('selection-selected', (event) => (selectedText.value = event.payload.text ?? '')))
  unlisteners.push(await onEvent<PublicSettings>('settings-changed', (event) => applySettings(event.payload)))
  const state = await command<ToolbarState>('toolbar_state')
  selectedText.value = state.selection?.text ?? ''
  applySettings(state.settings)
  if (toolbar.value) {
    resizeObserver = new ResizeObserver(reportSize)
    resizeObserver.observe(toolbar.value)
  }
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  unlisteners.forEach((unlisten) => unlisten())
  window.clearTimeout(copyTimer)
  window.clearTimeout(errorTimer)
})
</script>

<template>
  <main ref="toolbar" class="toolbar-stack">
    <div class="selection-toolbar" :class="{ compact: settings.compact }">
      <div class="toolbar-brand" data-tauri-drag-region><img :src="logoUrl" draggable="false" alt="" /></div>
      <div class="toolbar-actions">
        <button v-for="action in enabledActions" :key="action.id" type="button" :title="settings.compact ? action.name : ''" :aria-label="action.name" :class="{ success: copiedAction === action.id, busy: busyAction === action.id }" :disabled="Boolean(busyAction)" @click="perform(action.id)">
          <AppIcon :name="copiedAction === action.id ? 'check' : action.icon" :size="16" />
          <span v-if="!settings.compact">{{ action.name }}</span>
        </button>
      </div>
    </div>
    <div v-if="actionError" class="toolbar-error" role="alert">{{ actionError }}</div>
  </main>
</template>
