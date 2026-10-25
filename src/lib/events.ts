import { onScopeDispose } from 'vue'
import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event'
import { onEvent } from './bridge'

/** Dispose subscriptions, including those that resolve after the view closes. */
export function useAppEvents() {
  const listeners = new Set<UnlistenFn>()
  let disposed = false

  onScopeDispose(() => {
    disposed = true
    for (const unlisten of listeners) unlisten()
    listeners.clear()
  })

  return async <T>(name: string, callback: EventCallback<T>): Promise<boolean> => {
    const unlisten = await onEvent(name, callback)
    if (disposed) {
      unlisten()
      return false
    }
    listeners.add(unlisten)
    return true
  }
}
