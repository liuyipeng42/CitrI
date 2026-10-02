/** Batch frequent updates without postponing work indefinitely during a stream. */
export function coalescedTask(task: () => void, delayMs: number) {
  let timer: ReturnType<typeof setTimeout> | undefined

  function cancel(): void {
    clearTimeout(timer)
    timer = undefined
  }

  function flush(): void {
    cancel()
    task()
  }

  function schedule(): void {
    if (timer === undefined) timer = setTimeout(flush, delayMs)
  }

  return { schedule, flush, cancel }
}
