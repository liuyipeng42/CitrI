<script setup lang="ts">
import type { Settings } from '@/types'

const settings = defineModel<Settings>('settings', { required: true })
const filterList = defineModel<string>('filterList', { required: true })
</script>

<template>
  <section class="settings-card">
    <div class="section-title"><div><h2>应用过滤</h2><span>按进程名或应用名匹配，不区分大小写</span></div></div>
    <label class="field"><span>过滤模式</span><select v-model="settings.filterMode"><option value="default">默认排除列表 + 自定义排除</option><option value="blacklist">排除列表</option><option value="whitelist">仅允许列表</option></select></label>
    <label class="field filter-list"><span>{{ settings.filterMode === 'whitelist' ? '允许的应用' : '排除的应用' }}</span><textarea v-model="filterList" rows="10" placeholder="每行一个应用，例如：&#10;code.exe&#10;chrome" spellcheck="false" /><small>Windows 推荐填写完整进程名（如 <code>code.exe</code>），macOS/Linux 也可填写应用名。</small></label>
  </section>
</template>
