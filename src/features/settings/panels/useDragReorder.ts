import { nextTick, onBeforeUnmount, ref, type Ref } from 'vue'

export interface IdentifiableItem {
  id: string
}

interface DragState {
  id: string
  index: number
  startY: number
  active: boolean
  rects: DOMRect[]
  gap: number
}

export function useDragReorder<T extends IdentifiableItem>(
  items: Ref<T[]>,
  listEl: Ref<HTMLElement | null>,
  onReorder: () => void,
) {
  const draggingId = ref<string | null>(null)
  const dragIndex = ref(-1)
  const hoverIndex = ref(-1)
  const dragOffsetY = ref(0)

  const settlingId = ref<string | null>(null)
  const settleOffsetY = ref(0)
  const settlingTransition = ref(false)

  let drag: DragState | null = null
  let settleTimer: number | undefined
  let settleRaf: number | undefined

  function clearSettle(): void {
    window.clearTimeout(settleTimer)
    if (settleRaf !== undefined) cancelAnimationFrame(settleRaf)
    settleRaf = undefined
    settlingId.value = null
    settlingTransition.value = false
    settleOffsetY.value = 0
  }

  function updateDragPosition(clientY: number): void {
    if (!drag || !drag.active) return
    const { rects, index: from, startY } = drag
    const draggedRect = rects[from]
    const firstRect = rects[0]
    const lastRect = rects[rects.length - 1]
    if (!draggedRect || !firstRect || !lastRect) return

    const minOffset = firstRect.top - draggedRect.top - 12
    const maxOffset = lastRect.bottom - draggedRect.bottom + 12
    const dy = Math.min(maxOffset, Math.max(minOffset, clientY - startY))
    dragOffsetY.value = dy

    const dragTop = draggedRect.top + dy
    const dragBottom = draggedRect.bottom + dy
    let next = from
    if (dy > 0) {
      for (let i = from + 1; i < rects.length; i += 1) {
        const rect = rects[i]!
        if (dragBottom > rect.top + rect.height * 0.5) next = i
      }
    } else if (dy < 0) {
      for (let i = from - 1; i >= 0; i -= 1) {
        const rect = rects[i]!
        if (dragTop < rect.top + rect.height * 0.5) next = i
      }
    }
    hoverIndex.value = next
  }

  function onPointerMove(event: PointerEvent): void {
    if (!drag) return
    if (!drag.active) {
      if (Math.abs(event.clientY - drag.startY) < 4) return
      const container = listEl.value
      if (!container) return
      const rects = Array.from(container.children).map((el) => el.getBoundingClientRect())
      if (!rects[drag.index]) return
      clearSettle()
      drag.rects = rects
      drag.gap = rects.length > 1 ? Math.max(0, rects[1]!.top - rects[0]!.bottom) : 10
      drag.active = true
      draggingId.value = drag.id
      dragIndex.value = drag.index
      hoverIndex.value = drag.index
      document.body.style.userSelect = 'none'
      document.body.style.cursor = 'grabbing'
    }
    updateDragPosition(event.clientY)
  }

  function cleanupListeners(): void {
    window.removeEventListener('pointermove', onPointerMove)
    window.removeEventListener('pointerup', onPointerUp)
    window.removeEventListener('pointercancel', onPointerUp)
    document.body.style.userSelect = ''
    document.body.style.cursor = ''
  }

  async function onPointerUp(): Promise<void> {
    const finished = drag
    drag = null
    cleanupListeners()
    if (!finished?.active) return

    const from = finished.index
    const to = hoverIndex.value >= 0 ? hoverIndex.value : from
    const draggedRect = finished.rects[from]
    let targetShiftY = 0
    if (draggedRect && to > from && finished.rects[to]) {
      targetShiftY = finished.rects[to]!.bottom - draggedRect.bottom
    } else if (draggedRect && to < from && finished.rects[to]) {
      targetShiftY = finished.rects[to]!.top - draggedRect.top
    }
    const remainingY = dragOffsetY.value - targetShiftY

    draggingId.value = null
    dragIndex.value = -1
    hoverIndex.value = -1
    dragOffsetY.value = 0

    settlingId.value = finished.id
    settlingTransition.value = false
    settleOffsetY.value = remainingY

    if (to !== from) {
      const [item] = items.value.splice(from, 1)
      if (item) items.value.splice(to, 0, item)
      onReorder()
    }

    await nextTick()
    settleRaf = requestAnimationFrame(() => {
      settleRaf = requestAnimationFrame(() => {
        settlingTransition.value = true
        settleOffsetY.value = 0
      })
    })
    settleTimer = window.setTimeout(clearSettle, 220)
  }

  function startCardDrag(event: PointerEvent, id: string, index: number): void {
    if (event.button !== 0) return
    const target = event.target as HTMLElement | null
    if (!target) return
    const handle = target.closest('[data-drag-handle]')
    if (!handle && target.closest('input, textarea, button, select, [role="combobox"], [role="switch"], a')) return

    cleanupListeners()
    drag = { id, index, startY: event.clientY, active: false, rects: [], gap: 10 }
    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', onPointerUp)
    window.addEventListener('pointercancel', onPointerUp)
  }

  function itemWrapperStyle(id: string, index: number): Record<string, string> | undefined {
    if (draggingId.value === id) return { transform: `translate3d(0, ${dragOffsetY.value}px, 0)` }
    if (settlingId.value === id) return { transform: `translate3d(0, ${settleOffsetY.value}px, 0)` }
    if (draggingId.value !== null && drag) {
      const from = dragIndex.value
      const to = hoverIndex.value
      const step = (drag.rects[from]?.height ?? 0) + drag.gap
      if (from < to && index > from && index <= to) return { transform: `translate3d(0, ${-step}px, 0)` }
      if (from > to && index < from && index >= to) return { transform: `translate3d(0, ${step}px, 0)` }
      return { transform: 'translate3d(0, 0, 0)' }
    }
    return undefined
  }

  function itemWrapperClass(id: string): string {
    if (draggingId.value === id) return 'z-30'
    if (settlingId.value === id) {
      return settlingTransition.value
        ? 'z-20 transition-transform duration-200 ease-[cubic-bezier(0.22,1,0.36,1)]'
        : 'z-20'
    }
    if (draggingId.value !== null) return 'transition-transform duration-220 ease-[cubic-bezier(0.22,1,0.36,1)]'
    return ''
  }

  function moveBy(index: number, delta: number): void {
    const target = index + delta
    if (target < 0 || target >= items.value.length) return
    const [item] = items.value.splice(index, 1)
    if (item) {
      items.value.splice(target, 0, item)
      onReorder()
    }
  }

  onBeforeUnmount(() => {
    drag = null
    clearSettle()
    cleanupListeners()
  })

  return {
    draggingId,
    startCardDrag,
    itemWrapperStyle,
    itemWrapperClass,
    moveBy,
  }
}
