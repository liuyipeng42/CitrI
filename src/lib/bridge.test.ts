import { describe, expect, it } from 'vitest'
import { errorMessage } from './bridge'

describe('errorMessage', () => {
  it('keeps useful string and Error messages', () => {
    expect(errorMessage('连接失败')).toBe('连接失败')
    expect(errorMessage(new Error('请求超时'))).toBe('请求超时')
  })

  it('uses a stable fallback for unknown values', () => {
    expect(errorMessage({ code: 500 })).toBe('发生未知错误')
  })
})
