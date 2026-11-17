<script setup lang="ts">
import { computed } from 'vue'
import AppIcon from '@/components/AppIcon.vue'
import SettingToggle from '@/components/SettingToggle.vue'
import type { Settings } from '@/types'
import logoUrl from '@/assets/logo.png'

const settings = defineModel<Settings>({ required: true })
const enabledActions = computed(() => settings.value.actions.filter((action) => action.enabled))
</script>

<template>
  <section class="mx-auto flex max-w-[880px] flex-col gap-4 rounded-2xl border border-border bg-card p-6 shadow-xs">
    <SettingToggle v-model="settings.enabled" title="启用 CitrI" description="在其他应用中拖动或双击选中文本后自动显示工具条" />

    <div class="overflow-auto rounded-xl border border-border/80 bg-radial from-primary-soft/80 to-secondary p-6">
      <div class="mx-auto flex max-w-[560px] flex-col items-center gap-3.5 rounded-xl border border-border bg-card px-5 pb-5 pt-3.5 shadow-xs">
        <div class="flex gap-1.5 self-start">
          <span class="h-2.5 w-2.5 rounded-full bg-border" />
          <span class="h-2.5 w-2.5 rounded-full bg-border" />
          <span class="h-2.5 w-2.5 rounded-full bg-border" />
        </div>
        <div class="text-center text-[13px] leading-relaxed text-muted-foreground">
          <span>在任意文档或网页中，</span>
          <mark class="rounded bg-primary/18 px-1.5 py-0.5 text-foreground">选中这段文字</mark>
          <span>即可直接翻译、总结、解释或复制。</span>
        </div>
        <div
          class="inline-flex h-9 items-center gap-0.5 overflow-hidden whitespace-nowrap rounded-xl border border-border bg-card p-[3px] shadow-lg"
          :class="{ compact: settings.compact }"
        >
          <div class="mr-0.5 flex h-5.5 items-center justify-center border-r border-border pl-1 pr-2">
            <img :src="logoUrl" alt="" class="h-5 w-5 rounded-full object-cover" />
          </div>
          <button
            v-for="action in enabledActions"
            :key="action.id"
            type="button"
            :title="action.name"
            data-no-autosave
            :class="[
              'flex h-7 items-center justify-center gap-1 rounded-lg border-0 bg-transparent text-[12.5px] font-medium text-foreground transition-colors hover:bg-primary-soft hover:text-primary',
              settings.compact ? 'w-7 px-0' : 'px-2',
            ]"
          >
            <AppIcon :name="action.icon" :size="15" />
            <span v-if="!settings.compact">{{ action.name }}</span>
          </button>
        </div>
      </div>
    </div>

    <div class="grid grid-cols-2 gap-3 max-[820px]:grid-cols-1">
      <div class="flex flex-col gap-1 rounded-xl border border-border bg-secondary/75 px-3.5 py-3">
        <strong class="text-[12.5px] font-semibold text-foreground">拖选 / 双击唤出</strong>
        <span class="text-[11.5px] leading-relaxed text-muted-foreground">选区后自动定位至光标附近，不抢占当前窗口焦点</span>
      </div>
      <div class="flex flex-col gap-1 rounded-xl border border-border bg-secondary/75 px-3.5 py-3">
        <strong class="text-[12.5px] font-semibold text-foreground">多窗口与置顶</strong>
        <span class="text-[11.5px] leading-relaxed text-muted-foreground">结果支持流式 Markdown 输出，可置顶或调节透明度</span>
      </div>
    </div>
  </section>
</template>
