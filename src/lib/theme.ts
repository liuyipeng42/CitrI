import type { Settings } from '@/types'

type Appearance = Pick<Settings, 'accentColor' | 'customAccentColor' | 'fontSize'>

const accents = {
  cherry: { primary: '#ff5f57', soft: '#fff0ef', darkSoft: '#422421' },
  blue: { primary: '#3b82f6', soft: '#edf5ff', darkSoft: '#1d3557' },
  green: { primary: '#22a06b', soft: '#eaf8f1', darkSoft: '#193c30' },
  purple: { primary: '#8b5cf6', soft: '#f4efff', darkSoft: '#352850' },
  orange: { primary: '#f97316', soft: '#fff2e8', darkSoft: '#4a2b18' },
} as const

const darkScheme = window.matchMedia('(prefers-color-scheme: dark)')
let current: Appearance = { accentColor: 'orange', customAccentColor: '#f97316', fontSize: 100 }

export function normalizeHexColor(value: string, fallback = '#f97316'): string {
  const color = String(value || '').trim().toLowerCase()
  return /^#[0-9a-f]{6}$/.test(color) ? color : fallback
}

function mixColor(source: string, target: string, targetWeight: number): string {
  const from = normalizeHexColor(source).slice(1).match(/../g)!.map((part) => Number.parseInt(part, 16))
  const to = target.slice(1).match(/../g)!.map((part) => Number.parseInt(part, 16))
  const mixed = from.map((channel, index) => Math.round(channel * (1 - targetWeight) + to[index]! * targetWeight))
  return `#${mixed.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`
}

export function applyAppearance(next: Partial<Appearance> = {}): void {
  current = {
    accentColor: next.accentColor ?? current.accentColor,
    customAccentColor: next.customAccentColor ?? current.customAccentColor,
    fontSize: next.fontSize ?? current.fontSize,
  }
  const customPrimary = normalizeHexColor(current.customAccentColor)
  const accent = current.accentColor === 'custom'
    ? { primary: customPrimary, soft: mixColor(customPrimary, '#ffffff', 0.88), darkSoft: mixColor(customPrimary, '#000000', 0.65) }
    : accents[current.accentColor] ?? accents.orange
  const fontSize = Math.min(125, Math.max(85, Number(current.fontSize) || 100))
  const root = document.documentElement
  root.style.setProperty('--primary', accent.primary)
  root.style.setProperty('--primary-soft', darkScheme.matches ? accent.darkSoft : accent.soft)
  root.style.setProperty('--link', accent.primary)
  root.style.setProperty('--font-scale', String(fontSize / 100))
  root.dataset.accent = current.accentColor
}

const refreshAppearance = (): void => applyAppearance()
if (typeof darkScheme.addEventListener === 'function') darkScheme.addEventListener('change', refreshAppearance)
else (darkScheme as unknown as { addListener: (listener: () => void) => void }).addListener(refreshAppearance)
