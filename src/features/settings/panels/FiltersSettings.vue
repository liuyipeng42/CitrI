<script setup lang="ts">
import { computed } from 'vue'
import Badge from '@/components/ui/Badge.vue'
import type { FilterMode, Settings } from '@/types'

const settings = defineModel<Settings>('settings', { required: true })
const filterList = defineModel<string>('filterList', { required: true })

const modes: Array<{ id: FilterMode; title: string; description: string }> = [
  { id: 'default', title: '智能默认 + 自定义排除', description: '自动跳过常见截图与设计软件，同时排除下方列表' },
  { id: 'blacklist', title: '仅自定义排除', description: '仅屏蔽下方列表中的应用，其余全部响应' },
  { id: 'whitelist', title: '白名单模式', description: '仅在下方列表中的应用中响应划词' },
]

const ruleCount = computed(() => filterList.value.split(/\r?\n|,/).map((item) => item.trim()).filter(Boolean).length)
</script>

<template>
  <section class="mx-auto max-w-[880px] rounded-2xl border border-border bg-card p-6 shadow-xs">
    <div class="mb-4 flex items-start justify-between gap-3">
      <div>
        <h2 class="m-0 text-[15px] font-semibold tracking-tight text-foreground">应用过滤</h2>
        <span class="mt-1 block text-[11.5px] text-muted-foreground">按进程名或应用名匹配，不区分大小写</span>
      </div>
      <Badge variant="muted">{{ ruleCount }} 条规则</Badge>
    </div>

    <div class="mb-4 grid grid-cols-3 gap-2.5 max-[820px]:grid-cols-1" role="radiogroup" aria-label="过滤模式">
      <button
        v-for="mode in modes"
        :key="mode.id"
        type="button"
        :aria-checked="settings.filterMode === mode.id"
        role="radio"
        :class="[
          'flex flex-col gap-1 rounded-xl border p-3 text-left transition-all',
          settings.filterMode === mode.id
            ? 'border-primary bg-primary-soft shadow-2xs'
            : 'border-border bg-secondary/70 hover:border-primary/45',
        ]"
        @click="settings.filterMode = mode.id"
      >
        <strong :class="['text-[12.5px] font-semibold', settings.filterMode === mode.id ? 'text-primary' : 'text-foreground']">{{ mode.title }}</strong>
        <span class="text-[11px] leading-relaxed text-muted-foreground">{{ mode.description }}</span>
      </button>
    </div>

    <label class="flex flex-col gap-1.5 text-xs">
      <span class="font-semibold text-foreground">{{ settings.filterMode === 'whitelist' ? '允许的应用列表' : '排除的应用列表' }}</span>
      <textarea
        v-model="filterList"
        rows="10"
        placeholder="每行一个应用，例如：&#10;code.exe&#10;chrome"
        spellcheck="false"
        class="w-full resize-y rounded-lg border border-border bg-background px-3 py-2 font-mono text-xs leading-relaxed text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
      />
      <small class="text-[11px] text-muted-foreground">Windows 推荐填写完整进程名（如 <code class="rounded bg-secondary px-1 py-0.5 text-primary">code.exe</code>），macOS/Linux 也可填写应用名。</small>
    </label>
  </section>
</template>
