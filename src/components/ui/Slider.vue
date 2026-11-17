<script setup lang="ts">
import { computed, ref } from 'vue'
import { SliderRange, SliderRoot, SliderThumb, SliderTrack } from 'reka-ui'
import { cn } from '@/lib/utils'

const model = defineModel<number>({ required: true })
const props = withDefaults(defineProps<{
  min?: number
  max?: number
  step?: number
  disabled?: boolean
  class?: string
}>(), {
  min: 0,
  max: 100,
  step: 1,
})

const emit = defineEmits<{ change: [value: number] }>()
const wrapper = ref<HTMLElement | null>(null)

const sliderValue = computed({
  get: () => [model.value],
  set: (values: number[]) => {
    const next = values[0]
    if (next !== undefined) model.value = next
  },
})

function commitValue(values: number[]): void {
  const next = values[0]
  if (next !== undefined) emit('change', next)
  wrapper.value?.dispatchEvent(new Event('change', { bubbles: true }))
}
</script>

<template>
  <div ref="wrapper" :class="cn('w-full', props.class)">
    <SliderRoot
      v-model="sliderValue"
      :min="props.min"
      :max="props.max"
      :step="props.step"
      :disabled="props.disabled"
      class="relative flex w-full touch-none select-none items-center py-1.5"
      @value-commit="commitValue"
    >
      <SliderTrack class="relative h-1.5 w-full grow overflow-hidden rounded-full bg-muted">
        <SliderRange class="absolute h-full bg-primary" />
      </SliderTrack>
      <SliderThumb
        class="block h-4 w-4 rounded-full border-2 border-primary bg-card shadow-sm transition-transform hover:scale-110 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-primary/30 disabled:pointer-events-none disabled:opacity-50"
      />
    </SliderRoot>
  </div>
</template>
