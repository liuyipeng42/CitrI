<script setup lang="ts">
import { computed, nextTick } from 'vue'
import SettingToggle from '@/components/SettingToggle.vue'
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
  <section class="settings-card">
    <div class="section-title"><div><h2>工具条与结果窗口</h2><span>设置会即时应用到当前窗口</span></div></div>
    <SettingToggle v-model="settings.compact" title="紧凑模式" description="工具条只显示图标" /><div class="divider" />
    <SettingToggle v-model="settings.followToolbar" title="结果窗口跟随工具条" description="关闭时在当前屏幕中心打开" /><div class="divider" />
    <SettingToggle v-model="settings.autoClose" title="失去焦点时自动关闭" description="置顶窗口不会自动关闭" /><div class="divider" />
    <SettingToggle v-model="settings.autoPin" title="默认置顶结果窗口" description="新结果窗口保持在其他窗口上方" /><div class="divider" />
    <SettingToggle v-model="settings.rememberActionWindowSize" title="记住结果窗口大小" description="关闭窗口时保存当前大小，并在下次打开时恢复" /><div class="divider" />
    <label class="range-row"><span><strong>结果窗口透明度</strong><small>{{ settings.actionOpacity }}%</small></span><input v-model.number="settings.actionOpacity" type="range" min="20" max="100" /></label><div class="divider" />
    <label class="range-row"><span><strong>界面字号</strong><small>{{ fontSizeName }}</small></span><input v-model.number="settings.fontSize" type="range" min="85" max="125" step="5" @input="applyAppearance(settings)" /></label><div class="divider" />
    <div class="color-row"><div><strong>主题强调色</strong><p>用于按钮、链接和状态提示</p></div><div class="accent-colors"><button v-for="accent in accents" :key="accent.id" type="button" :class="{ active: settings.accentColor === accent.id }" :style="{ '--swatch': accent.id === 'custom' ? settings.customAccentColor : accent.color }" :title="accent.name" @click="chooseAccent(accent.id)" /><input id="custom-accent" v-model="settings.customAccentColor" type="color" tabindex="-1" @input="settings.accentColor = 'custom'; applyAppearance(settings)" /></div></div>
  </section>
</template>
