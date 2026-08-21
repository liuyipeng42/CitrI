import { describe, expect, it } from 'vitest'
import { normalizeExtraParameterValue } from './extraParameters'
import type { ExtraParameter } from '@/types'

function parameter(type: ExtraParameter['type'], value: string): ExtraParameter {
  return { key: 'thinking', type, value }
}

describe('normalizeExtraParameterValue', () => {
  it('replaces a previous boolean value when switching to JSON', () => {
    const item = parameter('json', 'false')
    normalizeExtraParameterValue(item)
    expect(item.value).toBe('{}')
  })

  it('preserves JSON objects entered by the user', () => {
    const item = parameter('json', '{"type":"disabled"}')
    normalizeExtraParameterValue(item)
    expect(item.value).toBe('{"type":"disabled"}')
  })

  it('preserves JSON arrays and null', () => {
    for (const value of ['[]', 'null']) {
      const item = parameter('json', value)
      normalizeExtraParameterValue(item)
      expect(item.value).toBe(value)
    }
  })
})
