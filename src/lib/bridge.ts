import { invoke } from '@tauri-apps/api/core'
import { listen, type EventCallback, type UnlistenFn } from '@tauri-apps/api/event'

export function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(name, args)
}

export function onEvent<T>(name: string, callback: EventCallback<T>): Promise<UnlistenFn> {
  return listen<T>(name, callback)
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  return typeof error === 'string' ? error : '发生未知错误'
}

