<script setup lang="ts">
import { computed, ref } from 'vue'
import AppIcon from '@/components/AppIcon.vue'
import { errorMessage } from '@/lib/bridge'
import { useSettingsEditor } from './useSettingsEditor'
import ActionsSettings from '@/features/settings/panels/ActionsSettings.vue'
import ApiSettings from '@/features/settings/api/ApiSettings.vue'
import AppearanceSettings from '@/features/settings/panels/AppearanceSettings.vue'
import FiltersSettings from '@/features/settings/panels/FiltersSettings.vue'
import GeneralSettings from '@/features/settings/panels/GeneralSettings.vue'
import HistorySettings from '@/features/history/HistorySettings.vue'
import SyncSettings from '@/features/sync/SyncSettings.vue'
import logoUrl from '@/assets/logo.png'
import './settings.css'


type Page = 'general' | 'api' | 'actions' | 'appearance' | 'history' | 'sync' | 'filters'

const pages: Array<{ id: Page; icon: string; title: string; description: string }> = [
  { id: 'general', icon: 'sparkles', title: '基础设置', description: '开启 CitrI 并预览工具条' },
  { id: 'api', icon: 'message-circle-question', title: 'API 与模型', description: '连接 OpenAI Chat Completions 兼容接口' },
  { id: 'actions', icon: 'align-left', title: '动作与 Prompt', description: '管理工具条动作、顺序与提示词' },
  { id: 'appearance', icon: 'droplet', title: '界面与窗口', description: '调整工具条和结果窗口的外观与行为' },
  { id: 'history', icon: 'history', title: '历史记录', description: '查看最近的 AI 请求内容与处理结果' },
  { id: 'sync', icon: 'refresh', title: '数据同步', description: 'ZIP 备份与恢复，WebDAV 多设备同步' },
  { id: 'filters', icon: 'search', title: '应用过滤', description: '控制在哪些应用中响应划词' },
]

const { settings, status, statusKind, serviceError, platformNotice, saving, hasUnsavedDraft, filterListText, apiSettings, setStatus, requestSave, prepareDataOperation, noteDraftEdit, noteLocalDraftChange, handleControlChange, handleControlFocusOut, handleControlClick } = useSettingsEditor()
const page = ref<Page>('general')
const dataBusy = ref(false)
const currentPage = computed(() => pages.find((item) => item.id === page.value) ?? pages[0]!)

function selectPage(next: Page): void {
  if (page.value === 'api') {
    try { apiSettings.value?.commit() }
    catch (error) { setStatus(errorMessage(error), 'error'); return }
    if (hasUnsavedDraft.value) requestSave()
  }
  page.value = next
}

</script>


<template>
  <div class="settings-shell">
    <aside class="settings-sidebar">
      <div class="sidebar-brand"><img :src="logoUrl" alt="" /><div><strong>CitrI</strong><span>设置</span></div></div>
      <nav class="sidebar-nav" aria-label="设置目录">
        <button v-for="item in pages" :key="item.id" type="button" :disabled="dataBusy" :class="{ active: page === item.id }" :aria-selected="page === item.id" @click="selectPage(item.id)">
          <AppIcon :name="item.icon" :size="17" /><span>{{ item.title }}</span>
        </button>
      </nav>
      <div class="sidebar-footer">Tauri 2 · Rust · Vue</div>
    </aside>

    <section class="settings-content">
      <header class="page-header" :class="{ 'history-page-header': page === 'history' }">
        <div><h1>{{ currentPage.title }}</h1><p v-if="page !== 'history'">{{ currentPage.description }}</p></div>
        <div class="header-actions"><span class="save-status" :class="statusKind">{{ status }}</span></div>
      </header>

      <main v-if="settings" class="settings-column" :class="{ 'history-page': page === 'history' }" @input="noteDraftEdit" @change="handleControlChange" @focusout="handleControlFocusOut" @click="handleControlClick">
        <div v-if="serviceError" class="notice error-notice">{{ serviceError }}</div>
        <div v-if="platformNotice" class="notice warning-notice">{{ platformNotice }}</div>

        <GeneralSettings v-if="page === 'general'" v-model="settings" />

        <ApiSettings ref="apiSettings" v-if="page === 'api'" v-model="settings" @status="setStatus" @draft-change="noteLocalDraftChange" />

        <ActionsSettings v-if="page === 'actions'" v-model="settings" />
        <AppearanceSettings v-if="page === 'appearance'" v-model="settings" />
        <HistorySettings v-if="page === 'history'" v-model="settings" />
        <SyncSettings v-if="page === 'sync'" :settings-saving="saving" :prepare="prepareDataOperation" @busy="dataBusy = $event" />
        <FiltersSettings v-if="page === 'filters'" v-model:settings="settings" v-model:filter-list="filterListText" />
      </main>

      <div v-else class="loading-state"><span class="spinner" />正在加载设置…</div>
    </section>
  </div>
</template>
