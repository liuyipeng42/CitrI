import { afterEach, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'

const { onEvent } = vi.hoisted(() => ({ onEvent: vi.fn() }))
vi.mock('./bridge', () => ({ onEvent }))
import { useAppEvents } from './events'

afterEach(() => onEvent.mockReset())

it('immediately releases a subscription that resolves after the view closes', async () => {
  let resolve!: (unlisten: UnlistenFn) => void
  onEvent.mockReturnValue(new Promise<UnlistenFn>((done) => { resolve = done }))
  const scope = effectScope()
  const listen = scope.run(useAppEvents)!
  const pending = listen('late', () => {})
  scope.stop()
  const unlisten = vi.fn()
  resolve(unlisten)
  expect(await pending).toBe(false)
  expect(unlisten).toHaveBeenCalledOnce()
})
