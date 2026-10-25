import { computed, onBeforeUnmount, ref, useTemplateRef, watch, type Ref } from 'vue'

export function useReasoning(loading: Ref<boolean>) {
  const reasoning = ref('')
  const reasoningExpanded = ref(true)
  const reasoningFinished = ref(false)
  const reasoningScroll = useTemplateRef<HTMLElement>('reasoningScroll')
  const reasoningActive = computed(() => loading.value && !reasoningFinished.value)

  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)')
  let reasoningFrame: number | undefined

  function finishReasoning(): void {
    if (reasoningFinished.value) return
    reasoningFinished.value = true
    reasoningExpanded.value = false
    if (reasoningFrame !== undefined) cancelAnimationFrame(reasoningFrame)
    reasoningFrame = undefined
  }

  function followReasoning(): void {
    if (reasoningFrame !== undefined || !reasoningActive.value || !reasoningExpanded.value) return
    let previous = performance.now()
    const step = (now: number): void => {
      reasoningFrame = undefined
      const viewport = reasoningScroll.value
      if (!viewport || !reasoningActive.value || !reasoningExpanded.value) return
      const target = Math.max(0, viewport.scrollHeight - viewport.clientHeight)
      const distance = target - viewport.scrollTop
      if (Math.abs(distance) < 1 || reducedMotion.matches) {
        viewport.scrollTop = target
        return
      }
      const movement = Math.max(1, Math.abs(distance) * (1 - Math.exp(-(now - previous) / 65)))
      viewport.scrollTop += Math.sign(distance) * Math.min(Math.abs(distance), movement)
      previous = now
      reasoningFrame = requestAnimationFrame(step)
    }
    reasoningFrame = requestAnimationFrame(step)
  }

  watch([reasoning, reasoningExpanded], followReasoning, { flush: 'post' })

  function resetReasoning(): void {
    reasoning.value = ''
    reasoningExpanded.value = true
    reasoningFinished.value = false
  }

  onBeforeUnmount(() => {
    if (reasoningFrame !== undefined) cancelAnimationFrame(reasoningFrame)
  })

  return { reasoning, reasoningExpanded, reasoningActive, finishReasoning, resetReasoning }
}
