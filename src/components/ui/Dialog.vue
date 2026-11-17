<script setup lang="ts">
import { DialogContent, DialogOverlay, DialogPortal, DialogRoot } from 'reka-ui'
import { cn } from '@/lib/utils'

const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{
  contentClass?: string
}>()
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal disabled>
      <DialogOverlay
        class="citri-dialog-overlay fixed inset-0 z-50 bg-black/45 backdrop-blur-xs"
      />
      <DialogContent
        :aria-describedby="undefined"
        :class="cn(
          'citri-dialog-content fixed left-1/2 top-1/2 z-50 flex max-h-[min(640px,calc(100vh-56px))] w-[min(740px,calc(100vw-48px))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-2xl border border-border bg-card shadow-2xl focus:outline-none',
          props.contentClass,
        )"
      >
        <slot />
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
