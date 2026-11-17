import { toRaw } from 'vue'
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'
import { command } from '@/lib/bridge'

const dateTimeFormatter = new Intl.DateTimeFormat('zh-CN', {
  year: 'numeric',
  month: 'numeric',
  day: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
  hour12: false,
})

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function cloneRaw<T>(value: T): T {
  return JSON.parse(JSON.stringify(toRaw(value))) as T
}

export function formatDateTime(timestamp: number): string {
  return dateTimeFormatter.format(timestamp)
}

export function openExternalLink(event: MouseEvent): void {
  const anchor = (event.target as HTMLElement | null)?.closest<HTMLAnchorElement>('a[href]')
  if (!anchor) return
  event.preventDefault()
  void command('open_target', { target: anchor.href })
}
