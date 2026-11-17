<script setup lang="ts">
import { SwitchRoot, SwitchThumb } from 'reka-ui'
import { cn } from '@/lib/utils'

const model = defineModel<boolean>({ required: true })
const props = defineProps<{ disabled?: boolean; class?: string }>()
</script>

<template>
  <SwitchRoot
    v-model="model"
    :disabled="props.disabled"
    :class="cn(
      'peer inline-flex h-[21px] w-[38px] shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent bg-muted-foreground/30 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/35 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary',
      props.class,
    )"
  >
    <SwitchThumb
      class="pointer-events-none block h-[15px] w-[15px] translate-x-[1px] rounded-full bg-white shadow-sm transition-transform duration-150 data-[state=checked]:translate-x-[17px]"
    />
  </SwitchRoot>
</template>
