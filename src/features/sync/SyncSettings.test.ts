// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'
import type { DataInfo, Settings } from '@/types'

const bridge = vi.hoisted(() => ({ command: vi.fn(), listeners: new Map<string, (event: { payload: unknown }) => void>() }))
vi.mock('@/lib/bridge', () => ({
  command: bridge.command,
  errorMessage: (reason: unknown) => reason instanceof Error ? reason.message : String(reason),
  onEvent: async (name: string, listener: (event: { payload: unknown }) => void) => { bridge.listeners.set(name, listener); return () => bridge.listeners.delete(name) },
}))
vi.mock('@/lib/theme', () => ({ applyAppearance: vi.fn() }))
import SettingsView from '@/features/settings/SettingsView.vue'

let root: HTMLElement
let app: App
let settings: Settings
let info: DataInfo

async function flush(): Promise<void> { for (let i = 0; i < 10; i += 1) { await Promise.resolve(); await nextTick() } }
function button(text: string): HTMLButtonElement {
  const found = [...root.querySelectorAll('button')].find((button) => button.textContent?.trim() === text)
  if (!found) throw new Error(`缺少按钮 ${text}`)
  return found
}
async function open(): Promise<void> { app = createApp(SettingsView); app.mount(root); await flush(); button('数据同步').click(); await flush() }

beforeEach(() => {
  root = document.createElement('div'); document.body.append(root)
  settings = { enabled: true, apiProfiles: [{ id: 'default', name: '默认', apiUrl: '', apiKey: '', model: 'test', systemPrompt: '', temperature: 0.3, maxTokens: 100, extraBody: {} }], activeApiProfileId: 'default', compact: false, followToolbar: true, autoClose: true, autoPin: false, rememberActionWindowSize: false, actionWindowWidth: 500, actionWindowHeight: 400, actionOpacity: 100, fontSize: 100, historyLimit: 100, accentColor: 'orange', customAccentColor: '#f97316', searchUrl: '', filterMode: 'default', filterList: [], actions: [] }
  info = { directory: 'C:/test/citri', historyCount: 3, preferences: { url: 'https://dav.example.com/CitrI/', username: 'user', password: 'test-password', autoSync: false, intervalMinutes: 5, includeApiKeys: false }, status: { busy: false, lastSyncedAt: 0, message: '', error: '' } }
  bridge.listeners.clear()
  bridge.command.mockReset().mockImplementation(async (name: string) => {
    if (name === 'get_settings') return structuredClone(settings)
    if (name === 'platform_info') return { platform: 'windows', displayProtocol: 'unknown' }
    if (name === 'get_data_info') return structuredClone(info)
    if (name === 'list_history') return { entries: [], total: 0, filteredTotal: 0 }
    if (name === 'set_sync_editing') return
    if (name === 'export_data') return { message: '已导出', historyCount: 3, path: 'backup.zip' }
    if (name === 'import_data') return { message: '已导入', historyCount: 4, path: 'before-import.zip' }
    throw new Error(`意外调用 ${name}`)
  })
})
afterEach(() => { app?.unmount(); root.remove() })

describe('数据同步页面', () => {
  it('exports with optional keys and confirms import before opening a file picker', async () => {
    await open(); button('导出 ZIP').click(); await flush()
    expect(bridge.command).toHaveBeenCalledWith('export_data', { includeApiKeys: false })
    root.querySelector<HTMLInputElement>('.sync-backup-actions input[type=checkbox]')!.click(); await flush()
    button('导出 ZIP').click(); await flush()
    expect(bridge.command).toHaveBeenCalledWith('export_data', { includeApiKeys: true })
    button('导入 ZIP').click(); await flush()
    expect(bridge.command).not.toHaveBeenCalledWith('import_data')
    button('选择 ZIP 并导入').click(); await flush()
    expect(bridge.command).toHaveBeenCalledWith('import_data')
    expect(root.querySelector('.sync-confirm')).toBeNull()
    expect(bridge.command.mock.calls.filter(([name]) => name === 'save_settings')).toHaveLength(0)
  })

  it('shows automatic sync status and errors and releases listeners after leaving', async () => {
    await open()
    bridge.listeners.get('sync-status')!({ payload: { busy: true, lastSyncedAt: 0, message: '正在同步', error: '' } }); await flush()
    expect(button('导出 ZIP').disabled).toBe(true)
    let resolveInfo!: (value: DataInfo) => void
    bridge.command.mockImplementationOnce(() => new Promise<DataInfo>((resolve) => { resolveInfo = resolve }))
    bridge.listeners.get('data-changed')!({ payload: null }); await flush()
    bridge.listeners.get('sync-status')!({ payload: { busy: false, lastSyncedAt: 123456789, message: '', error: 'WebDAV 身份验证失败' } }); await flush()
    resolveInfo({ ...info, status: { ...info.status, busy: true } }); await flush()
    expect(root.querySelector('.sync-settings .error-notice')?.textContent).toContain('身份验证失败')
    expect(button('导出 ZIP').disabled).toBe(false)
    button('应用过滤').click(); await flush()
    expect(bridge.listeners.has('sync-status')).toBe(false)
    expect(bridge.listeners.has('data-changed')).toBe(false)
  })

  it('applies imported settings while retaining an active local settings draft', async () => {
    await open()
    const imported = { ...settings, historyLimit: 60 }
    bridge.listeners.get('settings-restored')!({ payload: imported }); await flush()
    button('历史记录').click(); await flush()
    const historyInput = root.querySelector<HTMLInputElement>('.history-limit input')!
    expect(historyInput.value).toBe('60')
    historyInput.value = '25'; historyInput.dispatchEvent(new Event('input', { bubbles: true })); await flush()
    bridge.listeners.get('settings-restored')!({ payload: { ...settings, historyLimit: 80 } }); await flush()
    expect(historyInput.value).toBe('25')
    expect(bridge.command).toHaveBeenCalledWith('set_sync_editing', { editing: true })
  })
})
