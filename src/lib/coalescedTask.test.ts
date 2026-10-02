import { afterEach, expect, it, vi } from 'vitest'
import { coalescedTask } from './coalescedTask'

afterEach(() => vi.useRealTimers())

it('renders during a continuous stream instead of waiting for it to stop', () => {
  vi.useFakeTimers()
  const render = vi.fn()
  const task = coalescedTask(render, 50)
  for (let i = 0; i < 100; i++) {
    task.schedule()
    vi.advanceTimersByTime(10)
  }
  expect(render).toHaveBeenCalledTimes(20)
})

it('flushes the latest content immediately and cancels stale work', () => {
  vi.useFakeTimers()
  let content = 'partial'
  const render = vi.fn(() => content)
  const task = coalescedTask(render, 50)
  task.schedule()
  content = 'complete'
  task.flush()
  vi.runAllTimers()
  expect(render).toHaveBeenCalledTimes(1)
  expect(render).toHaveReturnedWith('complete')
  task.schedule()
  task.cancel()
  vi.runAllTimers()
  expect(render).toHaveBeenCalledTimes(1)
})
