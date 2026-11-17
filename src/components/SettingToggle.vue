<script setup lang="ts">
import Switch from '@/components/ui/Switch.vue'

defineProps<{ title: string; description: string }>()
const model = defineModel<boolean>({ required: true })
</script>

<template>
  <div class="flex min-h-12 items-center justify-between gap-5 py-1">
    <div class="space-y-1">
      <strong class="block text-[13px] font-semibold text-foreground">{{ title }}</strong>
      <p class="m-0 text-[11.5px] leading-relaxed text-muted-foreground">{{ description }}</p>
    </div>
    <Switch v-model="model" />
  </div>
</template>
