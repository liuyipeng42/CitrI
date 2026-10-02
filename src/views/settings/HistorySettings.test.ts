// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'
import type { HistoryEntry, Settings } from '@/types'

const bridge = vi.hoisted(() => ({
  command: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  unlisten: vi.fn(),
}))
vi.mock('@/lib/bridge', () => ({
  command: bridge.command,
  errorMessage: (error: unknown) => error instanceof Error ? error.message : String(error),
  onEvent: async (name: string, callback: (event: { payload: unknown }) => void) => {
    bridge.listeners.set(name, callback)
    return () => { bridge.listeners.delete(name); bridge.unlisten(name) }
  },
}))
vi.mock('@/lib/theme', () => ({ applyAppearance: vi.fn() }))
import SettingsView from '@/views/SettingsView.vue'

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
}

async function openHistory(): Promise<void> {
  app = createApp(SettingsView)
  app.mount(root)
  await flush()
  button('历史记录').click()
  await flush()
}

beforeEach(() => {
  root = document.createElement('div')
  document.body.append(root)
  records = [{
    id: 1, createdAt: Date.now(), actionName: '翻译', model: 'test-model', selectedText: 'hello',
    prompt: '翻译 hello', systemPrompt: '准确翻译', result: '**你好**<img src="x" onerror="alert(1)"><script>alert(1)</script>',
    status: 'success', error: null,
  }]
  bridge.listeners.clear()
  bridge.unlisten.mockClear()
  bridge.command.mockReset().mockImplementation(async (name: string, args?: { settings: Settings }) => {
    if (name === 'get_settings') return fixtureSettings()
    if (name === 'platform_info') return { platform: 'windows', displayProtocol: 'unknown' }
    if (name === 'get_history') return structuredClone(records)
    if (name === 'save_settings') return structuredClone(args!.settings)
    if (name === 'clear_history') { records = []; return }
    throw new Error(`意外调用：${name}`)
  })
})

afterEach(() => {
  app?.unmount()
  app = undefined
  root.remove()
})

describe('历史记录设置与交互', () => {
  it('places history below appearance and renders request details with sanitized Markdown', async () => {
    await openHistory()
    const navigation = [...root.querySelectorAll('.sidebar-nav button')].map((item) => item.textContent?.trim())
    expect(navigation.indexOf('历史记录')).toBe(navigation.indexOf('界面与窗口') + 1)
    expect(root.querySelector('.history-detail')?.textContent).toContain('翻译 hello')
    expect(root.querySelector('.history-result strong')?.textContent).toBe('你好')
    expect(root.querySelector('.history-result script')).toBeNull()
    expect(root.querySelector('.history-result img')?.hasAttribute('onerror')).toBe(false)
    expect(root.textContent).not.toContain('保留记录')
    expect(root.textContent).not.toContain('AI 请求结束后自动保存到本机，重启后仍可查看')
  })

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

  it('updates for new records, shows failed requests, and removes listeners when leaving history', async () => {
    await openHistory()
    records = [{ ...records[0]!, id: 2, status: 'error', result: '', error: '连接失败' }]
    bridge.listeners.get('history-changed')!({ payload: null })
    await flush()
    expect(root.querySelector('.history-detail')?.textContent).toContain('连接失败')
    button('界面与窗口').click()
    await flush()
    expect(bridge.listeners.has('history-changed')).toBe(false)
    expect(bridge.listeners.has('history-error')).toBe(false)
    expect(bridge.unlisten).toHaveBeenCalledWith('history-changed')
  })

  it('keeps records visible and reports a failed clear', async () => {
    await openHistory()
    bridge.command.mockImplementationOnce(async () => { throw new Error('磁盘不可写') })
    button('清空').click()
    await flush()
    button('确认清空').click()
    await flush()
    expect(root.querySelector('.error-notice')?.textContent).toBe('磁盘不可写')
    expect(root.querySelector('.history-detail')?.textContent).toContain('hello')
  })

  it('reports unreadable history and allows a confirmed reset', async () => {
    const implementation = bridge.command.getMockImplementation()!
    let damaged = true
    bridge.command.mockImplementation(async (name: string, args?: { settings: Settings }) => {
      if (name === 'get_history' && damaged) throw new Error('无法读取历史记录')
      if (name === 'clear_history') damaged = false
      return implementation(name, args)
    })
    await openHistory()
    expect(root.querySelector('.history-empty')?.textContent).toContain('无法加载历史记录')
    expect(button('清空').disabled).toBe(false)
    button('清空').click()
    await flush()
    button('确认清空').click()
    await flush()
    expect(root.querySelector('.history-empty')?.textContent).toContain('暂无历史记录')
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
    expect(bridge.command.mock.calls.filter(([name]) => name === 'get_history')).toHaveLength(1)
  })

  it('shows no matching results and restores all records when search is cleared', async () => {
    await openHistory()
    await search('没有匹配的关键词')
    expect(root.querySelector('.history-empty')?.textContent).toContain('没有找到匹配记录')
    expect(root.querySelector('.history-detail')).toBeNull()
    expect(root.querySelector('.history-count')?.textContent).toBe('0 / 1 条')
    root.querySelector<HTMLButtonElement>('[aria-label="清除搜索"]')!.click()
    await flush()
    expect(root.querySelector('.history-count')?.textContent).toBe('1 条')
    expect(root.querySelector('.history-detail')?.textContent).toContain('hello')
    expect(bridge.command.mock.calls.filter(([name]) => name === 'save_settings')).toHaveLength(0)
  })

  it('keeps search active when history changes and clears the search on Escape', async () => {
    await openHistory()
    await search('新增结果')
    records.unshift({ ...records[0]!, id: 2, selectedText: '新原文', result: '新增结果' })
    bridge.listeners.get('history-changed')!({ payload: null })
    await flush()
    expect(root.querySelectorAll('.history-list button')).toHaveLength(1)
    expect(root.querySelector('.history-detail')?.textContent).toContain('新原文')
    root.querySelector('.history-search input')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await flush()
    expect(root.querySelectorAll('.history-list button')).toHaveLength(2)
    expect(bridge.command.mock.calls.filter(([name]) => name === 'save_settings')).toHaveLength(0)
  })
})
