<script setup lang="ts">
import { nextTick, ref } from 'vue'
import {
  SelectContent,
  SelectItem,
  SelectItemIndicator,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectViewport,
} from 'reka-ui'
import { Check, ChevronDown } from '@lucide/vue'
import { cn } from '@/lib/utils'

export interface SelectOption {
  value: string
  label: string
}

const model = defineModel<string>({ required: true })
const props = defineProps<{
  options: SelectOption[]
  placeholder?: string
  ariaLabel?: string
  title?: string
  disabled?: boolean
  class?: string
}>()

const emit = defineEmits<{ change: [value: string] }>()
const wrapper = ref<HTMLElement | null>(null)

async function onUpdate(value: unknown): Promise<void> {
  const next = String(value ?? '')
  await nextTick()
  emit('change', next)
  wrapper.value?.dispatchEvent(new Event('change', { bubbles: true }))
}
</script>

<template>
  <div ref="wrapper" :class="cn('relative', props.class)">
    <SelectRoot v-model="model" :disabled="props.disabled" @update:model-value="onUpdate">
      <SelectTrigger
        data-no-autosave
        :aria-label="props.ariaLabel"
        :title="props.title"
        class="flex h-8 w-full items-center justify-between gap-2 rounded-lg border border-border bg-card px-2.5 text-xs text-foreground shadow-2xs transition-colors hover:border-primary/45 focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-50"
      >
        <SelectValue :placeholder="props.placeholder" class="truncate" />
        <ChevronDown :size="14" class="shrink-0 text-muted-foreground" />
      </SelectTrigger>

      <SelectPortal>
        <SelectContent
          position="popper"
          :side-offset="4"
          class="citri-select-content z-50 min-w-[var(--reka-select-trigger-width)] overflow-hidden rounded-lg border border-border bg-popover p-1 text-foreground shadow-lg"
        >
          <SelectViewport class="max-h-60 overflow-y-auto">
            <SelectItem
              v-for="option in props.options"
              :key="option.value"
              :value="option.value"
              class="relative flex cursor-pointer select-none items-center rounded-md py-1.5 pl-7 pr-2.5 text-xs outline-none data-[highlighted]:bg-primary-soft data-[highlighted]:text-primary data-[state=checked]:font-medium"
            >
              <SelectItemIndicator class="absolute left-2 inline-flex items-center justify-center text-primary">
                <Check :size="13" />
              </SelectItemIndicator>
              <SelectItemText>{{ option.label }}</SelectItemText>
            </SelectItem>
          </SelectViewport>
        </SelectContent>
      </SelectPortal>
    </SelectRoot>
  </div>
</template>
