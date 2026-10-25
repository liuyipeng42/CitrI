export type ExtraParameterType = 'string' | 'boolean' | 'number' | 'json'

export interface ExtraParameter {
  key: string
  type: ExtraParameterType
  value: string
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
