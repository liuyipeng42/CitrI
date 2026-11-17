<script setup lang="ts">
export type ResizeDirection =
  | 'north' | 'north-east' | 'east' | 'south-east'
  | 'south' | 'south-west' | 'west' | 'north-west'

const props = withDefaults(defineProps<{
  position?: 'fixed' | 'absolute'
  zClass?: string
}>(), {
  position: 'fixed',
  zClass: 'z-40',
})

const emit = defineEmits<{ resize: [direction: ResizeDirection] }>()

const handles: Array<{ direction: ResizeDirection; className: string }> = [
  { direction: 'north', className: 'top-0 left-2.5 right-2.5 h-1.5 cursor-n-resize' },
  { direction: 'south', className: 'bottom-0 left-2.5 right-2.5 h-1.5 cursor-s-resize' },
  { direction: 'east', className: 'right-0 top-2.5 bottom-2.5 w-1.5 cursor-e-resize' },
  { direction: 'west', className: 'left-0 top-2.5 bottom-2.5 w-1.5 cursor-w-resize' },
  { direction: 'north-west', className: 'top-0 left-0 h-2.5 w-2.5 cursor-nw-resize' },
  { direction: 'north-east', className: 'top-0 right-0 h-2.5 w-2.5 cursor-ne-resize' },
  { direction: 'south-west', className: 'bottom-0 left-0 h-2.5 w-2.5 cursor-sw-resize' },
  { direction: 'south-east', className: 'bottom-0 right-0 h-2.5 w-2.5 cursor-se-resize' },
]
</script>

<template>
  <div
    v-for="handle in handles"
    :key="handle.direction"
    :class="[props.position, props.zClass, handle.className]"
    @mousedown.left.prevent.stop="emit('resize', handle.direction)"
  />
</template>
