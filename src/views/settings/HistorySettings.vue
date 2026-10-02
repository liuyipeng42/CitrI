<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { renderMarkdown } from '@/lib/markdown'
import AppIcon from '@/components/AppIcon.vue'
import { command, errorMessage, onEvent } from '@/lib/bridge'
import { coalescedTask } from '@/lib/coalescedTask'
import type { HistoryEntry, HistoryPage, HistoryStatus, HistorySummary, Settings } from '@/types'

const settings = defineModel<Settings>({ required: true })
const entries = shallowRef<HistorySummary[]>([])
const selected = shallowRef<HistoryEntry | null>(null)
const query = ref('')
const total = ref(0)
const filteredTotal = ref(0)
const loading = ref(true)
const loadingMore = ref(false)
const refreshing = ref(false)
const searching = ref(false)
const clearing = ref(false)
const confirmingClear = ref(false)
const error = ref('')
const unlisteners: UnlistenFn[] = []
let disposed = false
let revision = 0
let detailRevision = 0
let searchTimer: number | undefined
const pageSize = 50
const renderedResult = computed(() => renderMarkdown(selected.value?.result ?? ''))
const statusNames: Record<HistoryStatus, string> = { success: '已完成', error: '失败', cancelled: '已停止' }

function formatDate(timestamp: number): string { return new Date(timestamp).toLocaleString('zh-CN', { hour12: false }) }
function updateLimit(event: Event): void {
  const input = event.target as HTMLInputElement
  const value = Number(input.value)
  const limit = input.value.trim() && Number.isFinite(value) ? Math.min(1000, Math.max(0, Math.trunc(value))) : settings.value.historyLimit
  settings.value.historyLimit = limit
  input.value = String(limit)
}

async function selectEntry(id: number): Promise<void> {
  if (selected.value?.id === id) return
  const current = ++detailRevision
  try {
    const entry = await command<HistoryEntry | null>('get_history_entry', { id })
    if (!disposed && current === detailRevision) selected.value = entry
  } catch (reason) { if (!disposed && current === detailRevision) error.value = errorMessage(reason) }
}

async function refresh(): Promise<void> {
  window.clearTimeout(searchTimer)
  const current = ++revision
  detailRevision += 1
  refreshing.value = true
  loadingMore.value = false
  try {
    const page = await command<HistoryPage>('list_history', { query: query.value, offset: 0, limit: pageSize })
    if (disposed || current !== revision) return
    entries.value = page.entries
    total.value = page.total
    filteredTotal.value = page.filteredTotal
    error.value = ''
    if (!page.entries.length) { detailRevision += 1; selected.value = null }
    else if (!page.entries.some((entry) => entry.id === selected.value?.id)) await selectEntry(page.entries[0]!.id)
  } catch (reason) { if (!disposed && current === revision) error.value = errorMessage(reason) }
  finally { if (!disposed && current === revision) { loading.value = false; searching.value = false; refreshing.value = false } }
}

async function loadMore(): Promise<void> {
  if (loadingMore.value || searching.value || refreshing.value || entries.value.length >= filteredTotal.value) return
  const current = revision
  loadingMore.value = true
  try {
    const page = await command<HistoryPage>('list_history', { query: query.value, offset: entries.value.length, limit: pageSize })
    if (disposed || current !== revision) return
    const existing = new Set(entries.value.map((entry) => entry.id))
    entries.value = [...entries.value, ...page.entries.filter((entry) => !existing.has(entry.id))]
    total.value = page.total
    filteredTotal.value = page.filteredTotal
  } catch (reason) { if (!disposed && current === revision) error.value = errorMessage(reason) }
  finally { if (!disposed && current === revision) loadingMore.value = false }
}

function scrollList(event: Event): void {
  const list = event.target as HTMLElement
  if (list.scrollHeight - list.scrollTop - list.clientHeight < 100) void loadMore()
}

watch(query, () => {
  window.clearTimeout(searchTimer)
  revision += 1
  detailRevision += 1
  searching.value = true
  searchTimer = window.setTimeout(() => { void refresh() }, 200)
})
const reload = coalescedTask(() => { void refresh() }, 80)

async function clear(): Promise<void> {
  clearing.value = true
  try { await command('clear_history'); confirmingClear.value = false; await refresh() }
  catch (reason) { error.value = errorMessage(reason) }
  finally { clearing.value = false }
}

onMounted(async () => {
  try {
    for (const [name, handler] of [
      ['history-changed', () => { reload.schedule() }],
      ['history-error', (event: { payload: unknown }) => { error.value = errorMessage(event.payload) }],
    ] as const) {
      const unlisten = await onEvent(name, handler)
      if (disposed) { unlisten(); return }
      unlisteners.push(unlisten)
    }
    await refresh()
  } catch (reason) { if (!disposed) { error.value = errorMessage(reason); loading.value = false } }
})

onBeforeUnmount(() => { disposed = true; revision += 1; detailRevision += 1; window.clearTimeout(searchTimer); reload.cancel(); unlisteners.forEach((unlisten) => unlisten()) })
</script>

<template>
  <section class="settings-card history-card">
    <div class="history-toolbar">
      <div class="history-search" @input.stop @change.stop @focusout.stop>
        <AppIcon name="search" :size="15" />
        <input v-model="query" type="search" aria-label="搜索历史记录" placeholder="搜索原文、请求或结果" autocomplete="off" spellcheck="false" @keydown.esc="query = ''" />
        <button v-if="query" class="icon-button" type="button" data-no-autosave title="清除搜索" aria-label="清除搜索" @click="query = ''"><AppIcon name="x" :size="14" /></button>
      </div>
      <label class="field history-limit">
        <span>保留</span>
        <input :value="settings.historyLimit" type="number" min="0" max="1000" step="1" aria-label="最多保留条数" aria-describedby="history-limit-tip" @change="updateLimit" />
        <span>条</span>
      </label>
      <div class="history-actions">
        <button class="secondary-button small-button" type="button" data-no-autosave :disabled="clearing" @click="refresh"><AppIcon name="refresh" :size="13" />刷新</button>
        <button class="secondary-button small-button" type="button" data-no-autosave :disabled="clearing || (!total && !error)" @click="confirmingClear = !confirmingClear"><AppIcon name="trash" :size="13" />清空</button>
      </div>
    </div>
    <div v-if="confirmingClear" class="history-confirm" role="alert">
      <span>清空全部历史记录？此操作无法撤销。</span>
      <div class="history-actions">
        <button class="secondary-button small-button" type="button" data-no-autosave :disabled="clearing" @click="confirmingClear = false">取消</button>
        <button class="secondary-button small-button history-danger" type="button" data-no-autosave :disabled="clearing" @click="clear">{{ clearing ? '正在清空…' : '确认清空' }}</button>
      </div>
    </div>
    <div v-if="error" class="notice error-notice" role="alert">{{ error }}</div>
    <div v-if="loading" class="history-empty"><span class="spinner" />正在加载历史记录…</div>
    <div v-else-if="error && !total" class="history-empty"><AppIcon name="history" :size="28" /><strong>无法加载历史记录</strong><span>可尝试刷新；若记录文件已损坏，可清空后重新记录</span></div>
    <div v-else-if="!total" class="history-empty"><AppIcon name="history" :size="28" /><strong>暂无历史记录</strong><span>{{ settings.historyLimit === 0 ? '保留条数设为 0，历史记录已关闭' : '使用翻译、总结等 AI 动作后，请求内容与处理结果会显示在这里' }}</span></div>
    <div v-else-if="!filteredTotal" class="history-empty"><AppIcon name="search" :size="28" /><strong>没有找到匹配记录</strong><span>试试其他关键词，或清除搜索查看全部记录</span><button class="secondary-button small-button" type="button" data-no-autosave @click="query = ''">查看全部记录</button></div>
    <div v-else class="history-layout">
      <nav class="history-list" aria-label="请求历史" @scroll="scrollList">
        <button v-for="entry in entries" :key="entry.id" type="button" data-no-autosave :class="{ active: selected?.id === entry.id }" :aria-current="selected?.id === entry.id ? 'true' : undefined" @click="selectEntry(entry.id)">
          <div class="history-item-heading"><strong>{{ entry.actionName }}</strong><span class="history-status" :class="entry.status">{{ statusNames[entry.status] }}</span></div>
          <p>{{ entry.preview }}</p><time :datetime="new Date(entry.createdAt).toISOString()">{{ formatDate(entry.createdAt) }}</time>
        </button>
        <button v-if="entries.length < filteredTotal" type="button" data-no-autosave :disabled="loadingMore || searching || refreshing" @click="loadMore">{{ loadingMore ? '正在加载…' : '加载更多' }}</button>
      </nav>
      <article v-if="selected" :key="selected.id" class="history-detail" aria-label="历史记录详情">
        <header class="history-detail-header">
          <div class="history-detail-heading"><h3 :title="selected.actionName">{{ selected.actionName }}</h3><span class="history-status" :class="selected.status">{{ statusNames[selected.status] }}</span></div>
          <div class="history-meta"><time :datetime="new Date(selected.createdAt).toISOString()">{{ formatDate(selected.createdAt) }}</time><span :title="selected.model">{{ selected.model }}</span></div>
        </header>
        <div class="history-detail-body">
          <details class="history-original">
            <summary><strong>原文</strong><span class="history-original-preview">{{ selected.selectedText }}</span><AppIcon name="chevron" :size="13" /></summary>
            <pre class="history-text">{{ selected.selectedText }}</pre>
          </details>
          <section class="history-answer" aria-label="处理结果">
            <h4>处理结果</h4>
            <div v-if="selected.status === 'cancelled'" class="history-note">请求已停止，以下为停止前返回的内容。</div>
            <div v-if="selected.error" class="notice error-notice">{{ selected.error }}</div>
            <div v-if="selected.result" class="history-result markdown" v-html="renderedResult" @click.prevent />
            <p v-else class="history-note">未返回处理结果。</p>
          </section>
          <details class="history-prompt"><summary>查看实际请求内容</summary><h4>系统 Prompt</h4><pre class="history-text">{{ selected.systemPrompt || '（未设置）' }}</pre><h4>用户请求</h4><pre class="history-text">{{ selected.prompt }}</pre></details>
        </div>
      </article>
    </div>
    <footer class="history-footer">
      <span class="history-count" aria-live="polite">{{ searching ? '正在搜索…' : query.trim() ? `${filteredTotal} / ${total} 条` : `${total} 条` }}</span>
      <span id="history-limit-tip" class="history-limit-tip" title="最多保留 1000 条；调小上限会立即删除最旧记录。">超限删除最旧记录</span>
    </footer>
  </section>
</template>

<style scoped>
.history-card { flex: 1; display: flex; flex-direction: column; min-width: 0; min-height: 0; width: 100%; max-width: none; margin: 0; padding: 0; overflow: hidden; box-shadow: none; }
.history-toolbar { display: flex; align-items: center; flex-wrap: wrap; flex-shrink: 0; gap: 10px; padding: 10px 12px; border-bottom: 1px solid var(--border); }
.history-limit { flex-direction: row; align-items: center; gap: 6px; flex-shrink: 0; }
.history-limit > span { font-weight: 400; color: var(--muted-foreground); white-space: nowrap; }
.history-limit input[type=number] { width: 66px; height: 32px; padding: 0 7px; }
.history-search { flex: 1 1 160px; display: flex; align-items: center; gap: 8px; min-width: 150px; height: 34px; padding: 0 9px; border: 1px solid var(--border); border-radius: 6px; color: var(--muted-foreground); background: var(--background); }
.history-search:focus-within { border-color: var(--primary); box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary), transparent 84%); }
.history-search input { flex: 1; min-width: 0; height: 100%; padding: 0; border: 0; background: transparent; box-shadow: none !important; color: var(--foreground); font-size: calc(12px * var(--font-scale)); }
.history-search input::-webkit-search-cancel-button { display: none; }
.history-search input::placeholder { color: var(--muted-foreground); }
.history-search .icon-button { width: 24px; height: 24px; padding: 0; border-radius: 4px; color: var(--muted-foreground); }
.history-search .icon-button:hover { color: var(--foreground); background: var(--accent); }
.history-actions { display: flex; gap: 6px; flex-shrink: 0; }
.history-confirm { flex-shrink: 0; display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px; padding: 9px 10px; margin: 10px 12px 0; border: 1px solid var(--border); border-radius: 6px; font-size: calc(12px * var(--font-scale)); }
.history-card > .notice { flex-shrink: 0; width: calc(100% - 24px); max-width: none; margin: 10px 12px 0; }
.history-danger { color: var(--error); }
.history-empty { flex: 1; min-height: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; padding: 24px; overflow: auto; color: var(--muted-foreground); text-align: center; font-size: calc(12px * var(--font-scale)); }
.history-empty strong { color: var(--foreground); font-size: calc(14px * var(--font-scale)); }
.history-layout { flex: 1; min-height: 0; display: grid; grid-template-columns: clamp(175px, 28%, 260px) minmax(0, 1fr); overflow: hidden; }
.history-list { min-height: 0; overflow: auto; border-right: 1px solid var(--border); background: var(--secondary); scrollbar-width: thin; }
.history-list button { width: 100%; display: block; padding: 10px 12px; border: 0; border-bottom: 1px solid var(--border); background: transparent; text-align: left; }
.history-list button:hover { background: var(--accent); }
.history-list button.active { background: var(--primary-soft); box-shadow: inset 3px 0 var(--primary); }
.history-item-heading, .history-detail-heading { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
.history-item-heading strong { font-size: calc(12px * var(--font-scale)); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.history-status { flex-shrink: 0; padding: 2px 5px; border-radius: 4px; font-size: calc(10px * var(--font-scale)); color: var(--muted-foreground); background: var(--muted); }
.history-status.success { color: var(--success); }
.history-status.error { color: var(--error); }
.history-list p { margin: 7px 0; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow-wrap: anywhere; font-size: calc(12px * var(--font-scale)); line-height: 1.5; }
.history-list time { color: var(--muted-foreground); font-size: calc(10px * var(--font-scale)); }
.history-detail { display: flex; flex-direction: column; min-width: 0; min-height: 0; overflow: hidden; }
.history-detail-header { flex-shrink: 0; padding: 12px 16px; border-bottom: 1px solid var(--border); }
.history-detail h3 { min-width: 0; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: calc(15px * var(--font-scale)); }
.history-detail h4 { margin: 14px 0 8px; font-size: calc(12px * var(--font-scale)); }
.history-meta, .history-note { color: var(--muted-foreground); font-size: calc(11px * var(--font-scale)); line-height: 1.6; overflow-wrap: anywhere; }
.history-meta { display: flex; align-items: center; gap: 10px; margin-top: 5px; }
.history-meta time { flex-shrink: 0; }
.history-meta > span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.history-detail-body { flex: 1; min-height: 0; padding: 12px 16px 16px; overflow: auto; scrollbar-width: thin; }
.history-original { border: 1px solid var(--border); border-radius: 6px; background: var(--secondary); overflow: hidden; }
.history-original summary { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 8px; padding: 8px 9px; cursor: pointer; list-style: none; font-size: calc(11px * var(--font-scale)); }
.history-original summary::-webkit-details-marker { display: none; }
.history-original-preview { color: var(--muted-foreground); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.history-original summary svg { color: var(--muted-foreground); transition: transform .15s; }
.history-original[open] summary svg { transform: rotate(180deg); }
.history-original[open] summary { border-bottom: 1px solid var(--border); }
.history-original .history-text { max-height: 240px; overflow: auto; border-radius: 0; }
.history-text { margin: 0; padding: 10px; border-radius: 6px; background: var(--secondary); font-family: inherit; font-size: calc(12px * var(--font-scale)); line-height: 1.65; white-space: pre-wrap; overflow-wrap: anywhere; }
.history-prompt { margin-top: 16px; padding-top: 12px; border-top: 1px solid var(--border); font-size: calc(11px * var(--font-scale)); }
.history-prompt summary { cursor: pointer; color: var(--primary); }
.history-result { font-size: calc(13px * var(--font-scale)); line-height: 1.65; overflow-wrap: anywhere; }
.history-footer { flex-shrink: 0; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 7px 12px; border-top: 1px solid var(--border); color: var(--muted-foreground); font-size: calc(10px * var(--font-scale)); line-height: 1.4; }
.history-count { flex-shrink: 0; }
.history-limit-tip { min-width: 0; text-align: right; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
@media (max-width: 820px) { .history-layout { grid-template-columns: 175px minmax(0, 1fr); } .history-detail-header, .history-detail-body { padding-left: 12px; padding-right: 12px; } }
</style>
