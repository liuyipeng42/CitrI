// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'
import type { HistoryEntry, Settings } from '@/types'

const bridge = vi.hoisted(() => ({
  command: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
}))
vi.mock('@/lib/bridge', () => ({
  command: bridge.command,
  errorMessage: (error: unknown) => error instanceof Error ? error.message : String(error),
  onEvent: async (name: string, callback: (event: { payload: unknown }) => void) => {
    bridge.listeners.set(name, callback)
    return () => { bridge.listeners.delete(name) }
  },
}))
vi.mock('@/lib/theme', () => ({ applyAppearance: vi.fn() }))
import SettingsView from '@/features/settings/SettingsView.vue'

let app: App | undefined
let root: HTMLElement
let records: HistoryEntry[]

function fixtureSettings(): Settings {
  return {
    enabled: true, apiProfiles: [{ id: 'default', name: '默认', apiUrl: '', apiKey: '', model: 'test', systemPrompt: '', temperature: 0.3, maxTokens: 100, extraBody: {} }],
    activeApiProfileId: 'default', compact: false, followToolbar: true, autoClose: true, autoPin: false,
    rememberActionWindowSize: false, actionWindowWidth: 500, actionWindowHeight: 400, actionOpacity: 100,
    fontSize: 100, historyLimit: 100, accentColor: 'orange', customAccentColor: '#f97316', searchUrl: '',
    filterMode: 'default', filterList: [], actions: [],
  }
}

async function flush(): Promise<void> {
  for (let index = 0; index < 8; index += 1) { await Promise.resolve(); await nextTick() }
}

function button(text: string): HTMLButtonElement {
  const result = [...root.querySelectorAll('button')].find((element) => element.textContent?.trim() === text)
  if (!result) throw new Error(`找不到按钮：${text}`)
  return result
}

async function search(text: string): Promise<void> {
  const input = root.querySelector<HTMLInputElement>('.history-search input')!
  input.value = text
  input.dispatchEvent(new Event('input', { bubbles: true }))
  input.dispatchEvent(new Event('change', { bubbles: true }))
  input.dispatchEvent(new FocusEvent('focusout', { bubbles: true }))
  await flush()
  await vi.advanceTimersByTimeAsync(220)
  await flush()
}

async function openHistory(): Promise<void> {
  app = createApp(SettingsView)
  app.mount(root)
  await flush()
  button('历史记录').click()
  await flush()
}

beforeEach(() => {
  vi.useFakeTimers()
  root = document.createElement('div')
  document.body.append(root)
  records = [{
    syncId: 'test-history-1', revivedAt: 0,
    id: 1, createdAt: Date.now(), actionName: '翻译', model: 'test-model', selectedText: 'hello',
    prompt: '翻译 hello', systemPrompt: '准确翻译', result: '**你好**<img src="x" onerror="alert(1)"><script>alert(1)</script>',
    status: 'success', error: null,
  }]
  bridge.listeners.clear()
  bridge.command.mockReset().mockImplementation(async (name: string, args?: { settings?: Settings; query?: string; offset?: number; limit?: number; id?: number }) => {
    if (name === 'get_settings') return fixtureSettings()
    if (name === 'platform_info') return { platform: 'windows', displayProtocol: 'unknown' }
    if (name === 'list_history') {
      const terms = (args?.query ?? '').trim().toLowerCase().split(/\s+/).filter(Boolean)
      const matches = records.filter((entry) => terms.every((term) => [entry.actionName, entry.model, entry.selectedText, entry.prompt, entry.systemPrompt, entry.result, entry.error ?? ''].join('\n').toLowerCase().includes(term)))
      return { total: records.length, filteredTotal: matches.length, entries: matches.slice(args?.offset ?? 0, (args?.offset ?? 0) + (args?.limit ?? 50)).map((entry) => ({ id: entry.id, createdAt: entry.createdAt, actionName: entry.actionName, preview: entry.selectedText.slice(0, 160), status: entry.status })) }
    }
    if (name === 'get_history_entry') return structuredClone(records.find((entry) => entry.id === args?.id) ?? null)
    if (name === 'save_settings') return structuredClone(args!.settings)
    if (name === 'clear_history') { records = []; return }
    throw new Error(`意外调用：${name}`)
  })
})

afterEach(() => {
  app?.unmount()
  app = undefined
  root.remove()
  vi.useRealTimers()
})

describe('历史记录设置与交互', () => {
  it('normalizes the retention count and automatically saves it on blur', async () => {
    await openHistory()
    const input = root.querySelector<HTMLInputElement>('.history-limit input')!
    input.value = '2000.7'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    input.dispatchEvent(new Event('change', { bubbles: true }))
    input.dispatchEvent(new FocusEvent('focusout', { bubbles: true }))
    await flush()
    expect(bridge.command).toHaveBeenCalledWith('save_settings', { settings: expect.objectContaining({ historyLimit: 1000 }) })
    expect(input.value).toBe('1000')
  })

  it('requires clear confirmation, updates empty state, and does not save settings on history clicks', async () => {
    await openHistory()
    button('刷新').click()
    await flush()
    button('清空').click()
    await flush()
    expect(bridge.command).not.toHaveBeenCalledWith('clear_history')
    button('取消').click()
    await flush()
    expect(root.querySelector('.history-confirm')).toBeNull()
    button('清空').click()
    await flush()
    button('确认清空').click()
    await flush()
    expect(bridge.command).toHaveBeenCalledWith('clear_history')
    expect(root.querySelector('.history-empty')?.textContent).toContain('暂无历史记录')
    expect(bridge.command.mock.calls.filter(([name]) => name === 'save_settings')).toHaveLength(0)
  })

  it('searches originals, prompts, results and metadata without changing saved settings', async () => {
    records.push({ ...records[0]!, id: 2, actionName: '总结', model: 'summary-model', selectedText: '另一段原文', prompt: '请求关键词', systemPrompt: '系统关键词', result: '回答关键词', error: '错误关键词' })
    await openHistory()
    await search('  HELLO  ')
    expect(root.querySelectorAll('.history-list button')).toHaveLength(1)
    expect(root.querySelector('.history-detail')?.textContent).toContain('hello')
    expect(root.querySelector('.history-count')?.textContent).toBe('1 / 2 条')
    for (const query of ['请求关键词', '系统关键词', '回答关键词', '错误关键词', '总结', 'SUMMARY-MODEL', '另一段 回答关键词']) {
      await search(query)
      expect(root.querySelectorAll('.history-list button')).toHaveLength(1)
      expect(root.querySelector('.history-list button.active')?.getAttribute('aria-current')).toBe('true')
      expect(root.querySelector('.history-detail')?.textContent).toContain('另一段原文')
    }
    expect(bridge.command.mock.calls.filter(([name]) => name === 'save_settings')).toHaveLength(0)
    expect(bridge.command.mock.calls.filter(([name]) => name === 'get_history_entry')).toHaveLength(2)
  })

  it('loads summaries in pages and reads only the selected detail', async () => {
    const prototype = records[0]!
    records = Array.from({ length: 95 }, (_, index) => ({ ...prototype, id: 95 - index, syncId: `record-${index}`, result: 'x'.repeat(50_000) }))
    await openHistory()
    expect(root.querySelectorAll('.history-item-heading')).toHaveLength(50)
    expect(bridge.command.mock.calls.filter(([name]) => name === 'get_history_entry')).toHaveLength(1)
    button('加载更多').click(); await flush()
    expect(root.querySelectorAll('.history-item-heading')).toHaveLength(95)
    expect(bridge.command).toHaveBeenCalledWith('list_history', { query: '', offset: 50, limit: 50 })
    expect(root.querySelector('.history-count')?.textContent).toBe('95 条')
  })
})
