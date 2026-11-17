import type { SelectOption } from '@/components/ui/Select.vue'
import type { JsonValue } from '@/types'

export type ExtraParameterType = 'string' | 'boolean' | 'number' | 'json'

export interface ExtraParameter {
  key: string
  type: ExtraParameterType
  value: string
}

export const paramTypeOptions: SelectOption[] = [
  { value: 'string', label: '字符串' },
  { value: 'boolean', label: '布尔值' },
  { value: 'number', label: '数字' },
  { value: 'json', label: 'JSON' },
]

export const boolOptions: SelectOption[] = [
  { value: 'true', label: 'true' },
  { value: 'false', label: 'false' },
]

export function parametersFromBody(body: Record<string, JsonValue>): ExtraParameter[] {
  return Object.entries(body).map(([key, value]) => {
    const type: ExtraParameterType = value === null || Array.isArray(value) || typeof value === 'object'
      ? 'json'
      : typeof value === 'boolean'
        ? 'boolean'
        : typeof value === 'number' ? 'number' : 'string'
    return { key, type, value: type === 'json' ? JSON.stringify(value) : String(value) }
  })
}

export function readExtraBody(parameters: ExtraParameter[]): Record<string, JsonValue> {
  const result: Record<string, JsonValue> = {}
  for (const parameter of parameters) {
    const key = parameter.key.trim()
    if (!key) continue
    if (Object.prototype.hasOwnProperty.call(result, key)) throw new Error(`额外请求参数名称重复：${key}`)
    if (parameter.type === 'boolean') result[key] = parameter.value === 'true'
    else if (parameter.type === 'number') {
      const value = Number(parameter.value)
      if (!parameter.value.trim() || !Number.isFinite(value)) throw new Error(`参数 ${key} 不是有效数字`)
      result[key] = value
    } else if (parameter.type === 'json') {
      try { result[key] = JSON.parse(parameter.value) }
      catch { throw new Error(`参数 ${key} 不是有效 JSON`) }
    } else result[key] = parameter.value
  }
  return result
}

export function normalizeExtraParameterValue(parameter: ExtraParameter): void {
  if (parameter.type === 'boolean') {
    if (!['true', 'false'].includes(parameter.value)) parameter.value = 'false'
    return
  }
  if (parameter.type === 'number') {
    if (!Number.isFinite(Number(parameter.value))) parameter.value = '0'
    return
  }
  if (parameter.type !== 'json') return
  try {
    const value: unknown = JSON.parse(parameter.value)
    if (value === null || Array.isArray(value) || typeof value === 'object') return
  } catch {
    // Replace values that are not valid JSON below.
  }
  parameter.value = '{}'
}
