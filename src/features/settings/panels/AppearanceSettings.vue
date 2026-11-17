<script setup lang="ts">
import { computed, nextTick } from 'vue'
import AppIcon from '@/components/AppIcon.vue'
import SettingToggle from '@/components/SettingToggle.vue'
import Badge from '@/components/ui/Badge.vue'
import Slider from '@/components/ui/Slider.vue'
import { applyAppearance } from '@/lib/theme'
import type { AccentColor, Settings } from '@/types'

const settings = defineModel<Settings>({ required: true })
const accents: Array<{ id: AccentColor; color: string; name: string }> = [
  { id: 'orange', color: '#f97316', name: '柑橘橙' },
  { id: 'cherry', color: '#ff5f57', name: '樱桃红' },
  { id: 'blue', color: '#3b82f6', name: '蓝色' },
  { id: 'green', color: '#22a06b', name: '绿色' },
  { id: 'purple', color: '#8b5cf6', name: '紫色' },
  { id: 'custom', color: '', name: '自定义' },
]

const fontSizeName = computed(() => {
  const names: Record<number, string> = { 85: '较小', 90: '小', 95: '稍小', 100: '标准', 105: '稍大', 110: '大', 115: '较大', 120: '特大', 125: '最大' }
  return `${names[settings.value.fontSize] ?? '自定义'} · ${settings.value.fontSize}%`
})

function chooseAccent(accent: AccentColor): void {
  settings.value.accentColor = accent
  applyAppearance(settings.value)
  if (accent === 'custom') nextTick(() => document.querySelector<HTMLInputElement>('#custom-accent')?.showPicker?.())
}
</script>

<template>
  <section class="mx-auto max-w-[880px] rounded-2xl border border-border bg-card p-6 shadow-xs">
    <div class="mb-3.5">
      <h2 class="m-0 text-[15px] font-semibold tracking-tight text-foreground">工具条与结果窗口</h2>
      <span class="mt-1 block text-[11.5px] text-muted-foreground">设置会即时应用到当前窗口</span>
    </div>

    <div class="divide-y divide-border/70">
      <SettingToggle v-model="settings.compact" title="紧凑模式" description="工具条只显示图标" class="py-2.5" />
      <SettingToggle v-model="settings.followToolbar" title="结果窗口跟随工具条" description="关闭时在当前屏幕中心打开" class="py-2.5" />
      <SettingToggle v-model="settings.autoClose" title="失去焦点时自动关闭" description="置顶窗口不会自动关闭" class="py-2.5" />
      <SettingToggle v-model="settings.autoPin" title="默认置顶结果窗口" description="新结果窗口保持在其他窗口上方" class="py-2.5" />
      <SettingToggle v-model="settings.rememberActionWindowSize" title="记住结果窗口大小" description="关闭窗口时保存当前大小，并在下次打开时恢复" class="py-2.5" />

      <div class="grid min-h-14 grid-cols-[196px_minmax(180px,1fr)] items-center gap-6 py-2.5">
        <div class="flex items-center justify-between gap-2">
          <strong class="text-[13px] font-semibold text-foreground">结果窗口透明度</strong>
          <Badge variant="default">{{ settings.actionOpacity }}%</Badge>
        </div>
        <Slider v-model="settings.actionOpacity" :min="20" :max="100" :step="1" />
      </div>

      <div class="grid min-h-14 grid-cols-[196px_minmax(180px,1fr)] items-center gap-6 py-2.5">
        <div class="flex flex-col gap-1">
          <strong class="text-[13px] font-semibold text-foreground">界面字号</strong>
          <small class="text-[11.5px] tabular-nums text-muted-foreground">{{ fontSizeName }}</small>
        </div>
        <Slider v-model="settings.fontSize" :min="85" :max="125" :step="5" @update:model-value="applyAppearance(settings)" />
      </div>

      <div class="flex min-h-14 items-center justify-between gap-5 pt-3">
        <div class="space-y-1">
          <strong class="block text-[13px] font-semibold text-foreground">主题强调色</strong>
          <p class="m-0 text-[11.5px] text-muted-foreground">用于按钮、链接和状态提示</p>
        </div>
        <div class="relative flex items-center gap-2.5">
          <button
            v-for="accent in accents"
            :key="accent.id"
            type="button"
            :style="{ backgroundColor: accent.id === 'custom' ? settings.customAccentColor : accent.color }"
            :title="accent.name"
            :class="[
              'grid h-7 w-7 place-items-center rounded-full text-white transition-transform hover:scale-110',
              accent.id === 'custom' && settings.accentColor !== 'custom' ? 'border-2 border-dashed border-border' : 'border border-black/10',
              settings.accentColor === accent.id ? 'scale-105 ring-2 ring-primary ring-offset-2 ring-offset-card' : '',
            ]"
            @click="chooseAccent(accent.id)"
          >
            <AppIcon v-if="settings.accentColor === accent.id" name="check" :size="13" class="drop-shadow-xs" />
          </button>
          <input
            id="custom-accent"
            v-model="settings.customAccentColor"
            type="color"
            tabindex="-1"
            class="pointer-events-none absolute h-px w-px opacity-0"
            @input="settings.accentColor = 'custom'; applyAppearance(settings)"
          />
        </div>
      </div>
    </div>
  </section>
</template>
