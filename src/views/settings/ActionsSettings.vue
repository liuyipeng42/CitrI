<script setup lang="ts">
import AppIcon from '@/components/AppIcon.vue'
import type { Settings } from '@/types'

const settings = defineModel<Settings>({ required: true })
const iconNames = ['copy', 'search', 'languages', 'align-left', 'message-circle-question', 'sparkles']

function moveAction(index: number, delta: number): void {
  const target = index + delta
  if (target < 0 || target >= settings.value.actions.length) return
  const [action] = settings.value.actions.splice(index, 1)
  if (action) settings.value.actions.splice(target, 0, action)
}

function addAction(): void {
  settings.value.actions.push({
    id: `custom-${Date.now()}`,
    name: '自定义', icon: 'sparkles', enabled: true, builtIn: false,
    prompt: '请处理下面的文本：\n\n{{text}}',
  })
}

function removeAction(index: number): void {
  if (!settings.value.actions[index]?.builtIn) settings.value.actions.splice(index, 1)
}
</script>

<template>
  <section class="settings-card">
    <div class="section-title action-heading"><div><h2>动作与 Prompt</h2><span>使用 <code v-pre>{{text}}</code> 插入选中文本</span></div><button class="secondary-button" type="button" @click="addAction"><AppIcon name="plus" :size="14" />添加自定义动作</button></div>
    <div class="actions-list">
      <article v-for="(action, index) in settings.actions" :key="action.id" class="action-card" :class="{ disabled: !action.enabled }">
        <label class="switch"><input v-model="action.enabled" type="checkbox" /><span /></label>
        <input v-model="action.name" class="action-name" maxlength="40" placeholder="动作名称" />
        <select v-model="action.icon" class="action-icon"><option v-for="icon in iconNames" :key="icon" :value="icon">{{ icon }}</option></select>
        <div class="action-controls"><button type="button" title="上移" :disabled="index === 0" @click="moveAction(index, -1)">↑</button><button type="button" title="下移" :disabled="index === settings.actions.length - 1" @click="moveAction(index, 1)">↓</button><button v-if="!action.builtIn" class="remove" type="button" title="删除" @click="removeAction(index)"><AppIcon name="trash" :size="14" /></button></div>
        <div v-if="action.id === 'copy'" class="action-hint">内置复制动作，不会调用 API。</div>
        <label v-else-if="action.id === 'search'" class="action-search"><span>搜索 URL</span><input v-model.trim="settings.searchUrl" spellcheck="false" /><small>使用 <code v-pre>{{queryString}}</code> 插入 URL 编码后的选中文本</small></label>
        <textarea v-else v-model="action.prompt" class="action-prompt" rows="4" placeholder="输入该动作的 Prompt" />
      </article>
    </div>
  </section>
</template>
