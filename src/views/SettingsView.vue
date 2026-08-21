<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, toRaw } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import AppIcon from '@/components/AppIcon.vue'
import { command, errorMessage, onEvent } from '@/lib/bridge'
import { applyAppearance } from '@/lib/theme'
import type { PlatformInfo, Settings } from '@/types'
import ActionsSettings from '@/views/settings/ActionsSettings.vue'
import ApiSettings from '@/views/settings/ApiSettings.vue'
import AppearanceSettings from '@/views/settings/AppearanceSettings.vue'
import FiltersSettings from '@/views/settings/FiltersSettings.vue'
import GeneralSettings from '@/views/settings/GeneralSettings.vue'
import logoUrl from '@/assets/logo.png'
import './settings.css'

type Page = 'general' | 'api' | 'actions' | 'appearance' | 'filters'

const pages: Array<{ id: Page; icon: string; title: string; description: string }> = [
  { id: 'general', icon: 'sparkles', title: '基础设置', description: '开启 CitrI 并预览工具条' },
  { id: 'api', icon: 'message-circle-question', title: 'API 与模型', description: '连接 OpenAI Chat Completions 兼容接口' },
  { id: 'actions', icon: 'align-left', title: '动作与 Prompt', description: '管理工具条动作、顺序与提示词' },
  { id: 'appearance', icon: 'droplet', title: '界面与窗口', description: '调整工具条和结果窗口的外观与行为' },
  { id: 'filters', icon: 'search', title: '应用过滤', description: '控制在哪些应用中响应划词' },
]
const settings = ref<Settings | null>(null)
const page = ref<Page>('general')
const status = ref('')
const statusKind = ref<'normal' | 'success' | 'error'>('normal')
const serviceError = ref('')
const platformNotice = ref('')
const saving = ref(false)
const filterListText = ref('')
const apiSettings = ref<InstanceType<typeof ApiSettings> | null>(null)
let statusTimer: number | undefined
let unlistenSelectionError: UnlistenFn | undefined
let editRevision = 0
let saveRequested = false
const editedFields = new WeakSet<Element>()

const currentPage = computed(() => pages.find((item) => item.id === page.value) ?? pages[0]!)

function setStatus(message: string, kind: 'normal' | 'success' | 'error' = 'normal'): void {
  window.clearTimeout(statusTimer)
  status.value = message
  statusKind.value = kind
  if (kind === 'success') statusTimer = window.setTimeout(() => (status.value = ''), 2600)
}

function requestSave(): void {
  editRevision += 1
  saveRequested = true
  void flushSaves()
}

function isTextField(target: EventTarget | null): target is HTMLInputElement | HTMLTextAreaElement {
  return target instanceof HTMLTextAreaElement
    || (target instanceof HTMLInputElement && !['checkbox', 'range', 'color'].includes(target.type))
}

function noteDraftEdit(event: Event): void {
  // Text fields save on change/blur, but typing must still invalidate an older
  // in-flight response so it cannot overwrite the unsaved draft.
  editRevision += 1
  if (isTextField(event.target)) editedFields.add(event.target)
}

function noteLocalDraftChange(): void {
  // API parameter rows live outside `settings` until ApiSettings commits them.
  // Invalidate any older save response so its reload cannot discard a new row.
  editRevision += 1
}

function handleControlChange(event: Event): void {
  if (!isTextField(event.target)) requestSave()
}

function handleControlFocusOut(event: FocusEvent): void {
  if (!isTextField(event.target) || !editedFields.has(event.target)) return
  editedFields.delete(event.target)
  requestSave()
}

function handleControlClick(event: MouseEvent): void {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button')
  if (button && !button.hasAttribute('data-no-autosave')) requestSave()
}

async function flushSaves(): Promise<void> {
  if (saving.value || !settings.value) return
  saving.value = true
  try {
    while (saveRequested && settings.value) {
      saveRequested = false
      const revision = editRevision
      setStatus('正在自动保存…')
      try {
        apiSettings.value?.commit()
        settings.value.filterList = filterListText.value.split(/\r?\n|,/).map((item) => item.trim()).filter(Boolean)
        const payload = JSON.parse(JSON.stringify(toRaw(settings.value))) as Settings
        const saved = await command<Settings>('save_settings', { settings: payload })

        // A newer edit may arrive while Rust is writing the previous snapshot.
        // Never replace that newer local state with an older response.
        if (editRevision === revision) {
          settings.value = saved
          filterListText.value = saved.filterList.join('\n')
          await nextTick()
          apiSettings.value?.reload()
          applyAppearance(saved)
          setStatus('已自动保存', 'success')
        } else if (!saveRequested) {
          setStatus('')
        }
      } catch (error) {
        setStatus(errorMessage(error), 'error')
        if (!saveRequested) break
      }
    }
  } finally {
    saving.value = false
    if (saveRequested) void flushSaves()
  }
}

onMounted(async () => {
  try {
    settings.value = await command<Settings>('get_settings')
    filterListText.value = settings.value.filterList.join('\n')
    applyAppearance(settings.value)
    const platform = await command<PlatformInfo>('platform_info')
    if (platform.platform === 'linux' && platform.displayProtocol === 'wayland') {
      platformNotice.value = 'Wayland 对全局鼠标监听有限制；如果桌面环境不支持所需协议，请切换到 X11/XWayland。'
    }
    unlistenSelectionError = await onEvent<string>('selection-error', (event) => (serviceError.value = event.payload ?? ''))
  } catch (error) { setStatus(errorMessage(error), 'error') }
})

onBeforeUnmount(() => {
  unlistenSelectionError?.()
  window.clearTimeout(statusTimer)
})
</script>

<template>
  <div class="settings-shell">
    <aside class="settings-sidebar">
      <div class="sidebar-brand"><img :src="logoUrl" alt="" /><div><strong>CitrI</strong><span>设置</span></div></div>
      <nav class="sidebar-nav" aria-label="设置目录">
        <button v-for="item in pages" :key="item.id" type="button" :class="{ active: page === item.id }" :aria-selected="page === item.id" @click="page = item.id">
          <AppIcon :name="item.icon" :size="17" /><span>{{ item.title }}</span>
        </button>
      </nav>
      <div class="sidebar-footer">Tauri 2 · Rust · Vue</div>
    </aside>

    <section class="settings-content">
      <header class="page-header">
        <div><h1>{{ currentPage.title }}</h1><p>{{ currentPage.description }}</p></div>
        <div class="header-actions"><span class="save-status" :class="statusKind">{{ status }}</span></div>
      </header>

      <main v-if="settings" class="settings-column" @input="noteDraftEdit" @change="handleControlChange" @focusout="handleControlFocusOut" @click="handleControlClick">
        <div v-if="serviceError" class="notice error-notice">{{ serviceError }}</div>
        <div v-if="platformNotice" class="notice warning-notice">{{ platformNotice }}</div>

        <GeneralSettings v-show="page === 'general'" v-model="settings" />

        <ApiSettings ref="apiSettings" v-show="page === 'api'" v-model="settings" @status="setStatus" @draft-change="noteLocalDraftChange" />

        <ActionsSettings v-show="page === 'actions'" v-model="settings" />
        <AppearanceSettings v-show="page === 'appearance'" v-model="settings" />
        <FiltersSettings v-show="page === 'filters'" v-model:settings="settings" v-model:filter-list="filterListText" />
      </main>

      <div v-else class="loading-state"><span class="spinner" />正在加载设置…</div>
    </section>
  </div>
</template>
