<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import AppIcon from '@/components/AppIcon.vue'
import WindowResizeHandles from '@/components/WindowResizeHandles.vue'
import Badge from '@/components/ui/Badge.vue'
import { command, errorMessage } from '@/lib/bridge'
import { useSettingsEditor } from './useSettingsEditor'
import ActionsSettings from '@/features/settings/panels/ActionsSettings.vue'
import ApiSettings from '@/features/settings/api/ApiSettings.vue'
import AppearanceSettings from '@/features/settings/panels/AppearanceSettings.vue'
import FiltersSettings from '@/features/settings/panels/FiltersSettings.vue'
import GeneralSettings from '@/features/settings/panels/GeneralSettings.vue'
import HistorySettings from '@/features/history/HistorySettings.vue'
import SyncSettings from '@/features/sync/SyncSettings.vue'
import logoUrl from '@/assets/logo.png'

type Page = 'general' | 'api' | 'actions' | 'appearance' | 'history' | 'sync' | 'filters'

const pages: Array<{ id: Page; icon: string; title: string; description: string }> = [
  { id: 'general', icon: 'sparkles', title: '基础设置', description: '开启 CitrI 并预览工具条' },
  { id: 'api', icon: 'cpu', title: 'API 与模型', description: '连接 OpenAI Chat Completions 兼容接口' },
  { id: 'actions', icon: 'command', title: '动作与 Prompt', description: '管理工具条动作、顺序与提示词' },
  { id: 'appearance', icon: 'palette', title: '界面与窗口', description: '调整工具条和结果窗口的外观与行为' },
  { id: 'history', icon: 'history', title: '历史记录', description: '查看最近的 AI 请求内容与处理结果' },
  { id: 'sync', icon: 'refresh', title: '数据同步', description: 'ZIP 备份与恢复，WebDAV 多设备同步' },
  { id: 'filters', icon: 'shield', title: '应用过滤', description: '控制在哪些应用中响应划词' },
]

const { settings, status, statusKind, serviceError, platformNotice, saving, hasUnsavedDraft, filterListText, apiSettings, setStatus, requestSave, prepareDataOperation, noteDraftEdit, noteLocalDraftChange, handleControlChange, handleControlFocusOut, handleControlClick } = useSettingsEditor()
const page = ref<Page>('general')
const dataBusy = ref(false)
const maximized = ref(false)
const currentPage = computed(() => pages.find((item) => item.id === page.value) ?? pages[0]!)
const statusBadgeVariant = computed(() => statusKind.value === 'success' ? 'success' : statusKind.value === 'error' ? 'destructive' : 'secondary')

function selectPage(next: Page): void {
  if (page.value === 'api') {
    try { apiSettings.value?.commit() }
    catch (error) { setStatus(errorMessage(error), 'error'); return }
    if (hasUnsavedDraft.value) requestSave()
  }
  page.value = next
}

async function minimizeWindow(): Promise<void> {
  await command('settings_window_control', { operation: 'minimize' }).catch(() => {})
}

async function toggleMaximize(): Promise<void> {
  maximized.value = await command<boolean>('settings_window_control', { operation: 'toggle-maximize' }).catch(() => maximized.value)
}

async function closeWindow(): Promise<void> {
  await command('settings_window_control', { operation: 'close' }).catch(() => {})
}

function startResize(direction: string): void {
  void command('settings_window_control', { operation: 'resize', direction }).catch(() => {})
}

function onTitlebarMousedown(event: MouseEvent): void {
  const target = event.target as HTMLElement | null
  if (target?.closest('button, a, input, select, textarea, [data-no-drag]')) return
  if (event.detail === 2) {
    void toggleMaximize()
  } else if (event.detail === 1) {
    void command('settings_window_control', { operation: 'drag' }).catch(() => {})
  }
}

function syncMaximized(): void {
  void command<boolean>('settings_window_control', { operation: 'is-maximized' })
    .then((value) => { maximized.value = Boolean(value) })
    .catch(() => {})
}

onMounted(() => {
  window.addEventListener('resize', syncMaximized)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', syncMaximized)
})
</script>

<template>
  <div class="relative flex h-screen w-full flex-col bg-muted/55 dark:bg-secondary">
    <WindowResizeHandles v-if="!maximized" position="fixed" z-class="z-40" @resize="startResize" />

    <header
      class="flex h-[44px] shrink-0 select-none items-center justify-between pl-4 pr-2.5"
      @mousedown.left="onTitlebarMousedown"
    >
      <div class="flex items-center gap-2.5">
        <img :src="logoUrl" draggable="false" alt="" class="h-7 w-7 rounded-full shadow-xs" />
        <div class="flex items-baseline gap-2">
          <span class="text-[15.5px] font-bold tracking-tight text-foreground">CitrI</span>
          <span class="text-[12.5px] text-muted-foreground">划词助手设置</span>
        </div>
      </div>

      <div class="flex items-center gap-1" aria-label="窗口控制" @mousedown.stop>
        <button
          type="button"
          data-no-autosave
          title="最小化"
          aria-label="最小化"
          class="grid h-8 w-8 place-items-center rounded-lg border-0 bg-transparent text-muted-foreground transition-all hover:bg-accent hover:text-foreground active:scale-95"
          @click="minimizeWindow"
        >
          <AppIcon name="minus" :size="14" />
        </button>
        <button
          type="button"
          data-no-autosave
          :title="maximized ? '向下还原' : '最大化'"
          :aria-label="maximized ? '向下还原' : '最大化'"
          class="grid h-8 w-8 place-items-center rounded-lg border-0 bg-transparent text-muted-foreground transition-all hover:bg-accent hover:text-foreground active:scale-95"
          @click="toggleMaximize"
        >
          <AppIcon :name="maximized ? 'copy' : 'square'" :size="12" />
        </button>
        <button
          type="button"
          data-no-autosave
          title="关闭到托盘"
          aria-label="关闭"
          class="grid h-8 w-8 place-items-center rounded-lg border-0 bg-transparent text-muted-foreground transition-all hover:bg-destructive hover:text-white active:scale-95"
          @click="closeWindow"
        >
          <AppIcon name="x" :size="15" />
        </button>
      </div>
    </header>

    <div class="flex min-h-0 flex-1">
      <aside class="flex w-[216px] shrink-0 select-none flex-col px-3 pb-4 pt-2 max-[820px]:w-[174px]">
        <nav class="flex flex-col gap-1" aria-label="设置目录">
          <button
            v-for="item in pages"
            :key="item.id"
            type="button"
            :disabled="dataBusy"
            :aria-selected="page === item.id"
            :class="[
              'relative flex h-9 items-center gap-2.5 rounded-lg px-3 text-left text-[13px] font-medium transition-colors',
              page === item.id
                ? 'bg-primary-soft font-semibold text-primary before:absolute before:bottom-2 before:left-0 before:top-2 before:w-[3px] before:rounded-r-sm before:bg-primary'
                : 'text-muted-foreground hover:bg-accent hover:text-foreground',
            ]"
            @click="selectPage(item.id)"
          >
            <AppIcon :name="item.icon" :size="16" />
            <span>{{ item.title }}</span>
          </button>
        </nav>

        <div class="mt-auto px-2.5 pt-3 text-[11px] tabular-nums text-muted-foreground/75">CitrI v0.3.5</div>
      </aside>

      <section class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-tl-xl border-l border-t border-border bg-background">
        <header class="flex h-[74px] shrink-0 items-center justify-between gap-5 border-b border-border bg-card/85 px-7 max-[820px]:px-4">
          <div class="min-w-0">
            <h1 class="m-0 text-[19px] font-semibold tracking-tight text-foreground">{{ currentPage.title }}</h1>
            <p class="mt-1 mb-0 text-xs text-muted-foreground">{{ currentPage.description }}</p>
          </div>
          <div class="flex items-center gap-3">
            <Badge v-if="status" :key="status" :variant="statusBadgeVariant" class="save-status animate-pop-in" :class="statusKind">{{ status }}</Badge>
          </div>
        </header>

        <main
          v-if="settings"
          class="min-h-0 flex-1 overflow-auto px-7 pb-11 pt-5 max-[820px]:px-4"
          :class="{ 'flex flex-col gap-3 overflow-hidden pb-5 pt-4': page === 'history' }"
          @input="noteDraftEdit"
          @change="handleControlChange"
          @focusout="handleControlFocusOut"
          @click="handleControlClick"
        >
          <div v-if="serviceError" class="animate-pop-in mx-auto mb-3.5 max-w-[880px] rounded-xl border border-destructive/40 bg-destructive/10 px-3.5 py-2.5 text-xs text-destructive">{{ serviceError }}</div>
          <div v-if="platformNotice" class="animate-pop-in mx-auto mb-3.5 max-w-[880px] rounded-xl border border-amber-400/70 bg-amber-50 px-3.5 py-2.5 text-xs text-amber-800 dark:border-amber-700/60 dark:bg-amber-950/50 dark:text-amber-200">{{ platformNotice }}</div>

          <GeneralSettings v-if="page === 'general'" v-model="settings" class="animate-panel-in" />
          <ApiSettings ref="apiSettings" v-if="page === 'api'" v-model="settings" class="animate-panel-in" @status="setStatus" @draft-change="noteLocalDraftChange" />
          <ActionsSettings v-if="page === 'actions'" v-model="settings" class="animate-panel-in" />
          <AppearanceSettings v-if="page === 'appearance'" v-model="settings" class="animate-panel-in" />
          <HistorySettings v-if="page === 'history'" v-model="settings" />
          <SyncSettings v-if="page === 'sync'" :settings-saving="saving" :prepare="prepareDataOperation" class="animate-panel-in" @busy="dataBusy = $event" />
          <FiltersSettings v-if="page === 'filters'" v-model:settings="settings" v-model:filter-list="filterListText" class="animate-panel-in" />
        </main>

        <div v-else class="flex flex-1 items-center justify-center gap-2.5 text-xs text-muted-foreground">
          <span class="spinner" />正在加载设置…
        </div>
      </section>
    </div>
  </div>
</template>
