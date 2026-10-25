// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import { createApp, nextTick } from 'vue'

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
import ActionView from './ActionView.vue'

it('renders streamed Markdown and flushes the final chunk on completion', async () => {
  vi.useFakeTimers()
  vi.stubGlobal('matchMedia', () => ({ matches: true }))
  const oldScroll = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'scrollTo')
  Object.defineProperty(HTMLElement.prototype, 'scrollTo', { configurable: true, value: vi.fn() })
  bridge.command.mockImplementation(async (name: string) => name === 'get_action_session' ? {
    action: { name: '总结', icon: 'sparkles' }, selectedText: '原文',
    settings: { autoPin: false, actionOpacity: 100 },
  } : undefined)
  const root = document.createElement('div')
  document.body.append(root)
  const app = createApp(ActionView)
  const flush = async () => { for (let i = 0; i < 8; i += 1) { await Promise.resolve(); await nextTick() } }
  try {
    app.mount(root)
    await flush()
    bridge.listeners.get('assistant-delta')!({ payload: '# 回答\n\n**重' })
    await vi.advanceTimersByTimeAsync(50)
    await flush()
    expect(root.querySelector('.result h1')?.textContent).toBe('回答')
    bridge.listeners.get('assistant-delta')!({ payload: '点**\n\n```ts\nconst n = 1\n```' })
    bridge.listeners.get('assistant-done')!({ payload: null })
    await flush()
    expect(root.querySelector('.result strong')?.textContent).toBe('重点')
    expect(root.querySelector('.result pre code.language-ts')?.textContent).toContain('const n = 1')
    expect(root.querySelector('.generating')).toBeNull()
  } finally {
    app.unmount()
    root.remove()
    if (oldScroll) Object.defineProperty(HTMLElement.prototype, 'scrollTo', oldScroll)
    else Reflect.deleteProperty(HTMLElement.prototype, 'scrollTo')
    vi.unstubAllGlobals()
    vi.useRealTimers()
  }
})
