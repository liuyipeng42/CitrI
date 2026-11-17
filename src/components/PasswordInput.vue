<script setup lang="ts">
import { ref } from 'vue'
import AppIcon from '@/components/AppIcon.vue'
import { cn } from '@/lib/utils'

const model = defineModel<string>({ required: true })
const props = withDefaults(defineProps<{
  placeholder?: string
  autocomplete?: string
  showTitle?: string
  hideTitle?: string
  inputClass?: string
}>(), {
  autocomplete: 'off',
  showTitle: '显示内容',
  hideTitle: '隐藏内容',
})

const visible = ref(false)
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)_38px]">
    <input
      v-model="model"
      :type="visible ? 'text' : 'password'"
      :placeholder="props.placeholder"
      :autocomplete="props.autocomplete"
      spellcheck="false"
      :class="cn(
        'h-9 w-full rounded-l-lg rounded-r-none border border-border bg-background px-3 text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20',
        props.inputClass,
      )"
    />
    <button
      type="button"
      data-no-autosave
      :title="visible ? props.hideTitle : props.showTitle"
      :aria-label="visible ? props.hideTitle : props.showTitle"
      class="grid place-items-center rounded-r-lg border border-l-0 border-border bg-secondary text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      @click="visible = !visible"
    >
      <AppIcon :name="visible ? 'eyeoff' : 'eye'" :size="15" />
    </button>
  </div>
</template>
