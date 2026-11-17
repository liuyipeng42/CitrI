<script setup lang="ts">
import AppIcon from '@/components/AppIcon.vue'
import type { Settings } from '@/types'

const settings = defineModel<Settings>({ required: true })
const iconOptions = [
  { value: 'copy', label: '双页图标' },
  { value: 'search', label: '放大镜图标' },
  { value: 'languages', label: '语言图标' },
  { value: 'align-left', label: '文本图标' },
  { value: 'message-circle-question', label: '问答气泡图标' },
  { value: 'sparkles', label: '星光图标' },
]

function moveAction(index: number, delta: number): void {
  const target = index + delta
  if (target < 0 || target >= settings.value.actions.length) return
  const [action] = settings.value.actions.splice(index, 1)
  if (action) settings.value.actions.splice(target, 0, action)
}

function addAction(): void {
  if (settings.value.actions.length >= 20) return
  settings.value.actions.push({
    id: `custom-${Date.now()}-${settings.value.actions.length + 1}`,
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
    <div class="section-title action-heading"><div><h2>动作与提示词</h2><span>自定义工具条按钮，让 AI 按你的要求处理选中文本</span></div><button class="secondary-button" type="button" :disabled="settings.actions.length >= 20" @click="addAction"><AppIcon name="plus" :size="14" />添加自定义动作</button></div>
    <p class="action-hint">不限于翻译、总结等预设功能。点击“添加自定义动作”，填写名称和提示词，即可创建润色、纠错、提取待办等 AI 功能。图标只影响按钮外观，不决定功能。</p>
    <p class="action-hint">提示词示例：请将下面的文本改写为礼貌、简洁的邮件：<code v-pre>{{text}}</code>。其中 <code v-pre>{{text}}</code> 会替换为选中的文本。自定义动作使用当前 API 配置。</p>
    <div class="actions-list">
      <article v-for="(action, index) in settings.actions" :key="action.id" class="action-card" :class="{ disabled: !action.enabled }">
        <label class="switch"><input v-model="action.enabled" type="checkbox" /><span /></label>
        <input v-model="action.name" class="action-name" maxlength="40" placeholder="动作名称" />
        <select v-model="action.icon" class="action-icon" aria-label="按钮图标（仅改变外观）" title="按钮图标：只改变外观，不影响动作功能"><option v-for="icon in iconOptions" :key="icon.value" :value="icon.value">{{ icon.label }}</option></select>
        <div class="action-controls"><button type="button" title="上移" :disabled="index === 0" @click="moveAction(index, -1)">↑</button><button type="button" title="下移" :disabled="index === settings.actions.length - 1" @click="moveAction(index, 1)">↓</button><button v-if="!action.builtIn" class="remove" type="button" title="删除" @click="removeAction(index)"><AppIcon name="trash" :size="14" /></button></div>
        <div v-if="action.id === 'copy'" class="action-hint">内置复制动作，不会调用 API。</div>
        <label v-else-if="action.id === 'search'" class="action-search"><span>搜索 URL</span><input v-model.trim="settings.searchUrl" spellcheck="false" /><small>使用 <code v-pre>{{queryString}}</code> 插入 URL 编码后的选中文本</small></label>
        <textarea v-else v-model="action.prompt" class="action-prompt" rows="4" aria-label="AI 动作提示词" placeholder="告诉 AI 如何处理选中文本，例如：纠正语法错误并说明修改原因。" />
      </article>
    </div>
  </section>
</template>
