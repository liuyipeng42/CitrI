import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch, type Ref } from 'vue'
import { useAppEvents } from '@/lib/events'
import { command, errorMessage } from '@/lib/bridge'
import { coalescedTask } from '@/lib/coalescedTask'
import type { HistoryEntry, HistoryPage, HistoryStatus, HistorySummary, Settings } from '@/types'

export const historyStatusNames: Record<HistoryStatus, string> = {
  success: '已完成',
  error: '失败',
  cancelled: '已停止',
}

export function historyStatusVariant(status: HistoryStatus) {
  return status === 'success' ? 'success' : status === 'error' ? 'destructive' : 'muted'
}

export function useHistorySettings(settings: Ref<Settings>) {
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

  const detailOpen = computed({
    get: () => activeId.value !== null,
    set: (value: boolean) => {
      if (!value) closeDetail()
    },
  })

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
    loadingDetail.value = false
    copied.value = ''
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
        selected.value = null
      } else if (activeId.value === null && selected.value !== null && !page.entries.some((entry) => entry.id === selected.value?.id)) {
        selected.value = null
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
      selected.value = null
      await refresh()
    } catch (reason) {
      error.value = errorMessage(reason)
    } finally {
      clearing.value = false
    }
  }

  onMounted(async () => {
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
    window.clearTimeout(searchTimer)
    window.clearTimeout(copyTimer)
    reload.cancel()
  })

  return {
    entries,
    selected,
    activeId,
    loadingDetail,
    query,
    total,
    filteredTotal,
    loading,
    loadingMore,
    refreshing,
    searching,
    clearing,
    confirmingClear,
    copied,
    error,
    detailOpen,
    updateLimit,
    copyText,
    openEntry,
    closeDetail,
    refresh,
    loadMore,
    scrollList,
    clear,
  }
}
