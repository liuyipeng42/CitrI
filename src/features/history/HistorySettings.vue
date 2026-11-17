<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { useAppEvents } from '@/lib/events'
import { renderMarkdown } from '@/lib/markdown'
import AppIcon from '@/components/AppIcon.vue'
import { command, errorMessage } from '@/lib/bridge'
import { coalescedTask } from '@/lib/coalescedTask'
import type { HistoryEntry, HistoryPage, HistoryStatus, HistorySummary, Settings } from '@/types'

const settings = defineModel<Settings>({ required: true })
const entries = shallowRef<HistorySummary[]>([])
const selected = shallowRef<HistoryEntry | null>(null)
const activeId = ref<number | null>(null)
const loadingDetail = ref(false)
const query = ref('')
const total = ref(0)
const filteredTotal = ref(0)
const loading = ref(true)
const loadingMore = ref(false)
const refreshing = ref(false)
const searching = ref(false)
const clearing = ref(false)
const confirmingClear = ref(false)
const copied = ref<'source' | 'result' | ''>('')
const error = ref('')
const listenEvent = useAppEvents()
let disposed = false
let revision = 0
let detailRevision = 0
let searchTimer: number | undefined
let copyTimer: number | undefined
const pageSize = 50
const renderedResult = computed(() => renderMarkdown(selected.value?.result ?? ''))
const statusNames: Record<HistoryStatus, string> = { success: '已完成', error: '失败', cancelled: '已停止' }

function formatDate(timestamp: number): string {
  return new Date(timestamp).toLocaleString('zh-CN', { hour12: false })
}

function updateLimit(event: Event): void {
  const input = event.target as HTMLInputElement
  const value = Number(input.value)
  const limit = input.value.trim() && Number.isFinite(value)
    ? Math.min(1000, Math.max(0, Math.trunc(value)))
    : settings.value.historyLimit
  settings.value.historyLimit = limit
  input.value = String(limit)
}

async function copyText(text: string, kind: 'source' | 'result'): Promise<void> {
  if (!text) return
  try {
    await command('clipboard_write', { text })
    copied.value = kind
    window.clearTimeout(copyTimer)
    copyTimer = window.setTimeout(() => { copied.value = '' }, 1200)
  } catch (reason) {
    error.value = errorMessage(reason)
  }
}

function openResultLink(event: MouseEvent): void {
  const anchor = (event.target as HTMLElement).closest<HTMLAnchorElement>('a[href]')
  if (!anchor) return
  event.preventDefault()
  void command('open_target', { target: anchor.href })
}

async function openEntry(id: number): Promise<void> {
  activeId.value = id
  copied.value = ''
  if (selected.value?.id === id) return
  selected.value = null
  loadingDetail.value = true
  const current = ++detailRevision
  try {
    const entry = await command<HistoryEntry | null>('get_history_entry', { id })
    if (!disposed && current === detailRevision) {
      selected.value = entry
      if (!entry) activeId.value = null
    }
  } catch (reason) {
    if (!disposed && current === detailRevision) {
      error.value = errorMessage(reason)
      activeId.value = null
    }
  } finally {
    if (!disposed && current === detailRevision) loadingDetail.value = false
  }
}

function closeDetail(): void {
  detailRevision += 1
  activeId.value = null
  selected.value = null
  loadingDetail.value = false
  copied.value = ''
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && activeId.value !== null) {
    event.stopPropagation()
    closeDetail()
  }
}

async function refresh(): Promise<void> {
  window.clearTimeout(searchTimer)
  const current = ++revision
  refreshing.value = true
  loadingMore.value = false
  try {
    const page = await command<HistoryPage>('list_history', { query: query.value, offset: 0, limit: pageSize })
    if (disposed || current !== revision) return
    entries.value = page.entries
    total.value = page.total
    filteredTotal.value = page.filteredTotal
    error.value = ''
    if (activeId.value !== null && !page.entries.some((entry) => entry.id === activeId.value)) {
      closeDetail()
    }
  } catch (reason) {
    if (!disposed && current === revision) error.value = errorMessage(reason)
  } finally {
    if (!disposed && current === revision) {
      loading.value = false
      searching.value = false
      refreshing.value = false
    }
  }
}

async function loadMore(): Promise<void> {
  if (loadingMore.value || searching.value || refreshing.value || entries.value.length >= filteredTotal.value) return
  const current = revision
  loadingMore.value = true
  try {
    const page = await command<HistoryPage>('list_history', {
      query: query.value,
      offset: entries.value.length,
      limit: pageSize,
    })
    if (disposed || current !== revision) return
    const existing = new Set(entries.value.map((entry) => entry.id))
    entries.value = [...entries.value, ...page.entries.filter((entry) => !existing.has(entry.id))]
    total.value = page.total
    filteredTotal.value = page.filteredTotal
  } catch (reason) {
    if (!disposed && current === revision) error.value = errorMessage(reason)
  } finally {
    if (!disposed && current === revision) loadingMore.value = false
  }
}

function scrollList(event: Event): void {
  const list = event.target as HTMLElement
  if (list.scrollHeight - list.scrollTop - list.clientHeight < 100) void loadMore()
}

watch(query, () => {
  window.clearTimeout(searchTimer)
  revision += 1
  searching.value = true
  searchTimer = window.setTimeout(() => { void refresh() }, 200)
})

const reload = coalescedTask(() => { void refresh() }, 80)

async function clear(): Promise<void> {
  clearing.value = true
  try {
    await command('clear_history')
    confirmingClear.value = false
    closeDetail()
    await refresh()
  } catch (reason) {
    error.value = errorMessage(reason)
  } finally {
    clearing.value = false
  }
}

onMounted(async () => {
  window.addEventListener('keydown', onKeydown)
  try {
    for (const [name, handler] of [
      ['history-changed', () => { reload.schedule() }],
      ['history-error', (event: { payload: unknown }) => { error.value = errorMessage(event.payload) }],
    ] as const) {
      if (!await listenEvent(name, handler)) return
    }
    await refresh()
  } catch (reason) {
    if (!disposed) {
      error.value = errorMessage(reason)
      loading.value = false
    }
  }
})

onBeforeUnmount(() => {
  disposed = true
  revision += 1
  detailRevision += 1
  window.removeEventListener('keydown', onKeydown)
  window.clearTimeout(searchTimer)
  window.clearTimeout(copyTimer)
  reload.cancel()
})
</script>

<template>
  <section class="settings-card history-card">
    <header class="history-toolbar">
      <div class="history-search" @input.stop @change.stop @focusout.stop>
        <AppIcon name="search" :size="15" />
        <input
          v-model="query"
          type="search"
          aria-label="搜索历史记录"
          placeholder="搜索原文、回答、提示词或模型…"
          autocomplete="off"
          spellcheck="false"
          @keydown.esc="query = ''"
        />
        <button v-if="query" class="icon-button" type="button" data-no-autosave title="清除搜索" aria-label="清除搜索" @click="query = ''">
          <AppIcon name="x" :size="14" />
        </button>
        <span class="history-count" aria-live="polite">{{ searching ? '正在搜索…' : query.trim() ? `${filteredTotal} / ${total} 条` : `${total} 条` }}</span>
      </div>

      <div class="history-controls">
        <label id="history-limit-tip" class="history-limit" title="最多保留 1000 条；超限自动清理最旧记录，设为 0 关闭历史记录">
          <span>保留</span>
          <input :value="settings.historyLimit" type="number" min="0" max="1000" step="1" aria-label="最多保留条数" aria-describedby="history-limit-tip" @change="updateLimit" />
          <span>条</span>
        </label>
        <button class="secondary-button small-button" type="button" data-no-autosave :disabled="clearing" @click="refresh">
          <AppIcon name="refresh" :size="13" />刷新
        </button>
        <button class="secondary-button small-button" type="button" data-no-autosave :disabled="clearing || (!total && !error)" @click="confirmingClear = !confirmingClear">
          <AppIcon name="trash" :size="13" />清空
        </button>
      </div>
    </header>

    <div v-if="confirmingClear" class="history-confirm" role="alert">
      <span>确定清空全部历史记录？此操作无法撤销。</span>
      <div class="history-confirm-actions">
        <button class="secondary-button small-button" type="button" data-no-autosave :disabled="clearing" @click="confirmingClear = false">取消</button>
        <button class="secondary-button small-button history-danger" type="button" data-no-autosave :disabled="clearing" @click="clear">{{ clearing ? '正在清空…' : '确认清空' }}</button>
      </div>
    </div>

    <div v-if="error" class="notice error-notice" role="alert">{{ error }}</div>

    <div v-if="loading" class="history-empty"><span class="spinner" />正在加载历史记录…</div>
    <div v-else-if="error && !total" class="history-empty">
      <div class="history-empty-badge"><AppIcon name="history" :size="24" /></div>
      <strong>无法加载历史记录</strong>
      <span>可尝试刷新；若记录文件已损坏，可清空后重新记录</span>
    </div>
    <div v-else-if="!total" class="history-empty">
      <div class="history-empty-badge"><AppIcon name="history" :size="24" /></div>
      <strong>暂无历史记录</strong>
      <span>{{ settings.historyLimit === 0 ? '保留条数设为 0，历史记录已关闭' : '使用翻译、总结等 AI 动作后，请求内容与处理结果会显示在这里' }}</span>
    </div>
    <div v-else-if="!filteredTotal" class="history-empty">
      <div class="history-empty-badge"><AppIcon name="search" :size="24" /></div>
      <strong>没有找到匹配记录</strong>
      <span>试试其他关键词，或清除搜索查看全部记录</span>
      <button class="secondary-button small-button" type="button" data-no-autosave @click="query = ''">查看全部记录</button>
    </div>

    <nav v-else class="history-list" aria-label="请求历史" @scroll="scrollList">
      <button
        v-for="entry in entries"
        :key="entry.id"
        class="history-item"
        type="button"
        data-no-autosave
        :class="{ active: activeId === entry.id }"
        :aria-current="activeId === entry.id ? 'true' : undefined"
        @click="openEntry(entry.id)"
      >
        <div class="history-item-main">
          <div class="history-item-heading">
            <strong>{{ entry.actionName }}</strong>
            <span class="history-status" :class="entry.status">{{ statusNames[entry.status] }}</span>
            <time :datetime="new Date(entry.createdAt).toISOString()">{{ formatDate(entry.createdAt) }}</time>
          </div>
          <p>{{ entry.preview }}</p>
        </div>
        <span class="history-item-arrow" aria-hidden="true"><AppIcon name="chevron" :size="15" /></span>
      </button>
      <button
        v-if="entries.length < filteredTotal"
        class="history-more"
        type="button"
        data-no-autosave
        :disabled="loadingMore || searching || refreshing"
        @click="loadMore"
      >
        {{ loadingMore ? '正在加载…' : '加载更多' }}
      </button>
    </nav>

    <div v-if="activeId !== null" class="history-modal-backdrop" @click.self="closeDetail">
      <article class="history-detail" role="dialog" aria-modal="true" aria-label="历史记录详情">
        <header class="history-detail-header">
          <div class="history-detail-title">
            <div class="history-detail-heading">
              <h3 :title="selected?.actionName">{{ selected?.actionName ?? '加载中…' }}</h3>
              <span v-if="selected" class="history-status" :class="selected.status">{{ statusNames[selected.status] }}</span>
            </div>
            <div v-if="selected" class="history-meta">
              <time :datetime="new Date(selected.createdAt).toISOString()">{{ formatDate(selected.createdAt) }}</time>
              <span class="history-meta-dot">·</span>
              <span class="history-model" :title="selected.model">{{ selected.model }}</span>
            </div>
          </div>
          <div class="history-detail-actions">
            <button
              v-if="selected?.result"
              class="secondary-button small-button"
              type="button"
              data-no-autosave
              @click="copyText(selected.result, 'result')"
            >
              <AppIcon :name="copied === 'result' ? 'check' : 'copy'" :size="13" />
              {{ copied === 'result' ? '已复制' : '复制结果' }}
            </button>
            <button class="icon-button history-modal-close" type="button" data-no-autosave title="关闭 (Esc)" aria-label="关闭详情" @click="closeDetail">
              <AppIcon name="x" :size="16" />
            </button>
          </div>
        </header>

        <div v-if="loadingDetail && !selected" class="history-detail-loading">
          <span class="spinner" />正在加载详情…
        </div>
        <div v-else-if="selected" :key="selected.id" class="history-detail-body">
          <section class="history-source" aria-label="选中文本">
            <div class="history-source-bar">
              <span>原文</span>
              <button type="button" data-no-autosave title="复制原文" @click="copyText(selected.selectedText, 'source')">
                <AppIcon :name="copied === 'source' ? 'check' : 'copy'" :size="12" />
                <span>{{ copied === 'source' ? '已复制' : '复制' }}</span>
              </button>
            </div>
            <div class="history-source-text">{{ selected.selectedText }}</div>
          </section>

          <section class="history-answer" aria-label="处理结果">
            <div v-if="selected.status === 'cancelled'" class="history-note">请求已停止，以下为停止前返回的内容。</div>
            <div v-if="selected.error" class="notice error-notice">{{ selected.error }}</div>
            <div v-if="selected.result" class="history-result markdown" v-html="renderedResult" @click="openResultLink" />
            <p v-else-if="!selected.error" class="history-note">未返回处理结果。</p>
          </section>

          <details class="history-prompt">
            <summary>
              <span>查看实际请求 Prompt</span>
              <AppIcon name="chevron" :size="13" />
            </summary>
            <div class="history-prompt-grid">
              <div>
                <h4>系统 Prompt</h4>
                <pre class="history-text">{{ selected.systemPrompt || '（未设置）' }}</pre>
              </div>
              <div>
                <h4>用户请求</h4>
                <pre class="history-text">{{ selected.prompt }}</pre>
              </div>
            </div>
          </details>
        </div>
      </article>
    </div>
  </section>
</template>

<style scoped src="./history.css" />
