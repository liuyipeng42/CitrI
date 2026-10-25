// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import { createApp, nextTick } from 'vue'
import type { ToolbarState } from '@/types'

const bridge = vi.hoisted(() => ({ command: vi.fn(), listeners: new Map<string, (event: { payload: unknown }) => void>() }))
vi.mock('@/lib/bridge', () => ({
  command: bridge.command,
  errorMessage: String,
  onEvent: async (name: string, callback: (event: { payload: unknown }) => void) => {
    bridge.listeners.set(name, callback)
    return () => bridge.listeners.delete(name)
  },
}))
vi.mock('@/lib/theme', () => ({ applyAppearance: vi.fn() }))
import ToolbarView from './ToolbarView.vue'

it('keeps a new selection when an older initial snapshot arrives and delegates text to Rust', async () => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  let resolveState!: (state: ToolbarState) => void
  const pendingState = new Promise<ToolbarState>((resolve) => { resolveState = resolve })
  bridge.command.mockImplementation(async (name: string) => name === 'toolbar_state' ? pendingState : true)
  const root = document.createElement('div')
  document.body.append(root)
  const app = createApp(ToolbarView)
  const flush = async () => { for (let i = 0; i < 8; i += 1) { await Promise.resolve(); await nextTick() } }
  try {
    app.mount(root)
    await flush()
    bridge.listeners.get('selection-selected')!({ payload: { text: '最新选区' } })
    resolveState({
      selection: null,
      settings: { compact: false, autoClose: true, autoPin: false, actionOpacity: 100,
        fontSize: 100, accentColor: 'orange', customAccentColor: '#f97316',
        actions: [{ id: 'translate', name: '翻译', icon: 'sparkles', enabled: true, builtIn: true }] },
    })
    await flush()
    root.querySelector<HTMLButtonElement>('button')!.click()
    await flush()
    expect(bridge.command).toHaveBeenCalledWith('perform_action', { actionId: 'translate' })
    expect(bridge.command.mock.calls.filter(([name]) => name === 'perform_action')).toHaveLength(1)
    bridge.listeners.get('selection-selected')!({ payload: { text: '' } })
    root.querySelector<HTMLButtonElement>('button')!.click()
    await flush()
    expect(bridge.command.mock.calls.filter(([name]) => name === 'perform_action')).toHaveLength(1)
  } finally {
    app.unmount()
    root.remove()
    vi.unstubAllGlobals()
  }
})
