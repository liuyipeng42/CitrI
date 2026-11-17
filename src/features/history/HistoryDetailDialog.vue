<script setup lang="ts">
import { computed } from 'vue'
import { DialogClose, DialogTitle } from 'reka-ui'
import AppIcon from '@/components/AppIcon.vue'
import Badge from '@/components/ui/Badge.vue'
import Button from '@/components/ui/Button.vue'
import Dialog from '@/components/ui/Dialog.vue'
import { renderMarkdown } from '@/lib/markdown'
import { formatDateTime, openExternalLink } from '@/lib/utils'
import { historyStatusNames, historyStatusVariant } from './useHistorySettings'
import type { HistoryEntry } from '@/types'

const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{
  selected: HistoryEntry | null
  loading: boolean
  copied: 'source' | 'result' | ''
}>()
const emit = defineEmits<{
  copy: [text: string, kind: 'source' | 'result']
  close: []
}>()

const renderedResult = computed(() => renderMarkdown(props.selected?.result ?? ''))
</script>

<template>
  <Dialog v-model:open="open" content-class="history-detail">
    <header class="flex shrink-0 items-center justify-between gap-3 border-b border-border bg-secondary/50 px-5 py-3.5">
      <div class="min-w-0">
        <div class="flex items-center gap-2.5">
          <DialogTitle as-child>
            <h3 :title="selected?.actionName" class="m-0 truncate text-[15.5px] font-semibold tracking-tight text-foreground">
              {{ selected?.actionName ?? '加载中…' }}
            </h3>
          </DialogTitle>
          <Badge v-if="selected" :variant="historyStatusVariant(selected.status)">{{ historyStatusNames[selected.status] }}</Badge>
        </div>
        <div v-if="selected" class="mt-1 flex items-center gap-1.5 text-[11px] tabular-nums text-muted-foreground">
          <time :datetime="new Date(selected.createdAt).toISOString()" class="shrink-0">{{ formatDateTime(selected.createdAt) }}</time>
          <span class="shrink-0">·</span>
          <span class="truncate" :title="selected.model">{{ selected.model }}</span>
        </div>
      </div>

      <div class="flex shrink-0 items-center gap-2">
        <Button
          v-if="selected?.result"
          size="sm"
          variant="secondary"
          data-no-autosave
          @click="emit('copy', selected.result, 'result')"
        >
          <AppIcon :name="copied === 'result' ? 'check' : 'copy'" :size="13" />
          {{ copied === 'result' ? '已复制' : '复制结果' }}
        </Button>
        <DialogClose as-child>
          <Button
            size="icon"
            variant="ghost"
            data-no-autosave
            title="关闭 (Esc)"
            aria-label="关闭详情"
            class="history-modal-close h-8 w-8"
            @click="emit('close')"
          >
            <AppIcon name="x" :size="16" />
          </Button>
        </DialogClose>
      </div>
    </header>

    <div v-if="loading && !selected" class="flex items-center justify-center gap-2.5 px-6 py-12 text-xs text-muted-foreground">
      <span class="spinner" />正在加载详情…
    </div>

    <div v-else-if="selected" :key="selected.id" class="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-5 pb-5 pt-4">
      <section class="shrink-0 rounded-r-lg border-l-[3px] border-primary bg-secondary/75 px-3.5 py-2.5" aria-label="选中文本">
        <div class="mb-1.5 flex items-center justify-between text-[11px] font-semibold text-muted-foreground">
          <span>原文</span>
          <button
            type="button"
            data-no-autosave
            title="复制原文"
            class="inline-flex items-center gap-1 rounded-md border-0 bg-transparent px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground hover:bg-accent hover:text-foreground"
            @click="emit('copy', selected.selectedText, 'source')"
          >
            <AppIcon :name="copied === 'source' ? 'check' : 'copy'" :size="12" />
            <span>{{ copied === 'source' ? '已复制' : '复制' }}</span>
          </button>
        </div>
        <div class="max-h-32 overflow-auto whitespace-pre-wrap break-words text-[12.5px] leading-relaxed text-foreground">
          {{ selected.selectedText }}
        </div>
      </section>

      <section class="min-w-0 shrink-0" aria-label="处理结果">
        <div v-if="selected.status === 'cancelled'" class="mb-2 text-xs text-muted-foreground">请求已停止，以下为停止前返回的内容。</div>
        <div v-if="selected.error" class="mb-3 rounded-xl border border-destructive/40 bg-destructive/10 px-3.5 py-2.5 text-xs text-destructive">{{ selected.error }}</div>
        <div v-if="selected.result" class="markdown text-[13.5px] leading-relaxed" v-html="renderedResult" @click="openExternalLink" />
        <p v-else-if="!selected.error" class="m-0 text-xs text-muted-foreground">未返回处理结果。</p>
      </section>

      <details class="group shrink-0 border-t border-border pt-3.5 text-[11.5px]">
        <summary class="inline-flex cursor-pointer list-none select-none items-center gap-1.5 font-medium text-muted-foreground hover:text-foreground [&::-webkit-details-marker]:hidden">
          <span>查看实际请求 Prompt</span>
          <AppIcon name="chevron" :size="13" class="transition-transform group-open:rotate-180" />
        </summary>
        <div class="mt-2.5 flex flex-col gap-2.5">
          <div>
            <h4 class="mb-1 mt-0 text-[11px] font-semibold text-muted-foreground">系统 Prompt</h4>
            <pre class="m-0 whitespace-pre-wrap break-words rounded-lg border border-border bg-secondary/70 px-3 py-2.5 font-sans text-xs leading-relaxed">{{ selected.systemPrompt || '（未设置）' }}</pre>
          </div>
          <div>
            <h4 class="mb-1 mt-0 text-[11px] font-semibold text-muted-foreground">用户请求</h4>
            <pre class="m-0 whitespace-pre-wrap break-words rounded-lg border border-border bg-secondary/70 px-3 py-2.5 font-sans text-xs leading-relaxed">{{ selected.prompt }}</pre>
          </div>
        </div>
      </details>
    </div>
  </Dialog>
</template>
