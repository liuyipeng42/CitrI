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
  <section class="settings-card">
    <div class="section-title"><div><h2>CitrI</h2><span>轻量、常驻、随选随用</span></div></div>
    <SettingToggle v-model="settings.enabled" title="启用 CitrI" description="在其他应用中选中文本后显示工具条" />
    <div class="divider" />
    <div class="preview-area">
      <div class="toolbar-preview" :class="{ compact: settings.compact }">
        <div class="preview-brand"><img :src="logoUrl" alt="" /></div>
        <button v-for="action in enabledActions" :key="action.id" type="button" :title="action.name"><AppIcon :name="action.icon" :size="15" /><span v-if="!settings.compact">{{ action.name }}</span></button>
      </div>
    </div>
    <p class="card-tip">保存后，在任意应用中拖动选中文本或双击单词即可唤出工具条。</p>
  </section>
</template>
