<script setup lang="ts">
import AppIcon from '@/components/AppIcon.vue'
import Badge from '@/components/ui/Badge.vue'
import Button from '@/components/ui/Button.vue'
import { formatDateTime } from '@/lib/utils'
import HistoryDetailDialog from './HistoryDetailDialog.vue'
import { historyStatusNames, historyStatusVariant, useHistorySettings } from './useHistorySettings'
import type { Settings } from '@/types'

const settings = defineModel<Settings>({ required: true })
const {
  entries,
  selected,
  activeId,
  loadingDetail,
  query,
  total,
  filteredTotal,
  loading,
  loadingMore,
  refreshing,
  searching,
  clearing,
  confirmingClear,
  copied,
  error,
  detailOpen,
  updateLimit,
  copyText,
  openEntry,
  closeDetail,
  refresh,
  loadMore,
  scrollList,
  clear,
} = useHistorySettings(settings)
</script>

<template>
  <div class="flex min-h-0 w-full flex-1 flex-col">
    <section class="animate-panel-in mx-auto flex min-h-0 w-full max-w-[880px] flex-1 flex-col overflow-hidden rounded-2xl border border-border bg-card shadow-xs">
      <header class="flex shrink-0 flex-wrap items-center justify-between gap-2.5 border-b border-border bg-card px-4 py-3">
        <div
          class="history-search flex h-8.5 min-w-[180px] flex-1 basis-[220px] items-center gap-2 rounded-lg border border-border bg-secondary/70 pl-2.5 pr-1.5 text-muted-foreground transition-colors focus-within:border-primary focus-within:bg-card focus-within:ring-2 focus-within:ring-primary/20"
          @input.stop
          @change.stop
          @focusout.stop
        >
          <AppIcon name="search" :size="15" />
          <input
            v-model="query"
            type="search"
            aria-label="搜索历史记录"
            placeholder="搜索原文、回答、提示词或模型…"
            autocomplete="off"
            spellcheck="false"
            class="h-full min-w-0 flex-1 border-0 bg-transparent p-0 text-xs text-foreground shadow-none outline-none placeholder:text-muted-foreground [&::-webkit-search-cancel-button]:hidden"
            @keydown.esc="query = ''"
          />
          <button
            v-if="query"
            type="button"
            data-no-autosave
            title="清除搜索"
            aria-label="清除搜索"
            class="inline-flex h-5.5 w-5.5 items-center justify-center rounded-md border-0 bg-transparent text-muted-foreground hover:bg-accent hover:text-foreground"
            @click="query = ''"
          >
            <AppIcon name="x" :size="14" />
          </button>
          <Badge variant="muted" class="history-count rounded-md bg-card" aria-live="polite">
            {{ searching ? '正在搜索…' : query.trim() ? `${filteredTotal} / ${total} 条` : `${total} 条` }}
          </Badge>
        </div>

        <div class="flex shrink-0 items-center gap-2">
          <label
            id="history-limit-tip"
            class="history-limit inline-flex h-8.5 items-center gap-1.5 whitespace-nowrap rounded-lg border border-border bg-secondary/70 px-2.5 text-xs text-muted-foreground"
            title="最多保留 1000 条；超限自动清理最旧记录，设为 0 关闭历史记录"
          >
            <span>保留</span>
            <input
              :value="settings.historyLimit"
              type="number"
              min="0"
              max="1000"
              step="1"
              aria-label="最多保留条数"
              aria-describedby="history-limit-tip"
              class="h-5.5 w-13 rounded-md border border-border bg-card px-1 text-center text-xs font-semibold tabular-nums text-foreground"
              @change="updateLimit"
            />
            <span>条</span>
          </label>
          <Button variant="secondary" class="h-8.5" data-no-autosave :disabled="clearing" @click="refresh">
            <AppIcon name="refresh" :size="13" />刷新
          </Button>
          <Button variant="secondary" class="h-8.5" data-no-autosave :disabled="clearing || (!total && !error)" @click="confirmingClear = !confirmingClear">
            <AppIcon name="trash" :size="13" />清空
          </Button>
        </div>
      </header>

      <div v-if="confirmingClear" class="history-confirm animate-panel-in flex shrink-0 flex-wrap items-center justify-between gap-2.5 border-b border-destructive/30 bg-destructive/8 px-4 py-2.5 text-xs" role="alert">
        <span class="text-foreground">确定清空全部历史记录？此操作无法撤销。</span>
        <div class="flex items-center gap-2">
          <Button size="sm" variant="secondary" data-no-autosave :disabled="clearing" @click="confirmingClear = false">取消</Button>
          <Button size="sm" variant="destructive" data-no-autosave :disabled="clearing" @click="clear">{{ clearing ? '正在清空…' : '确认清空' }}</Button>
        </div>
      </div>

      <div v-if="error" class="mx-4 mt-3 shrink-0 rounded-xl border border-destructive/40 bg-destructive/10 px-3.5 py-2.5 text-xs text-destructive" role="alert">{{ error }}</div>

      <div v-if="loading" class="history-empty flex min-h-0 flex-1 flex-col items-center justify-center gap-2 p-10 text-xs text-muted-foreground">
        <span class="spinner" />正在加载历史记录…
      </div>
      <div v-else-if="error && !total" class="history-empty flex min-h-0 flex-1 flex-col items-center justify-center gap-2 p-10 text-center text-xs text-muted-foreground">
        <div class="mb-1 grid h-12 w-12 place-items-center rounded-2xl bg-primary-soft text-primary"><AppIcon name="history" :size="24" /></div>
        <strong class="text-sm text-foreground">无法加载历史记录</strong>
        <span>可尝试刷新；若记录文件已损坏，可清空后重新记录</span>
      </div>
      <div v-else-if="!total" class="history-empty flex min-h-0 flex-1 flex-col items-center justify-center gap-2 p-10 text-center text-xs text-muted-foreground">
        <div class="mb-1 grid h-12 w-12 place-items-center rounded-2xl bg-primary-soft text-primary"><AppIcon name="history" :size="24" /></div>
        <strong class="text-sm text-foreground">暂无历史记录</strong>
        <span>{{ settings.historyLimit === 0 ? '保留条数设为 0，历史记录已关闭' : '使用翻译、总结等 AI 动作后，请求内容与处理结果会显示在这里' }}</span>
      </div>
      <div v-else-if="!filteredTotal" class="history-empty flex min-h-0 flex-1 flex-col items-center justify-center gap-2 p-10 text-center text-xs text-muted-foreground">
        <div class="mb-1 grid h-12 w-12 place-items-center rounded-2xl bg-primary-soft text-primary"><AppIcon name="search" :size="24" /></div>
        <strong class="text-sm text-foreground">没有找到匹配记录</strong>
        <span>试试其他关键词，或清除搜索查看全部记录</span>
        <Button size="sm" variant="secondary" data-no-autosave class="mt-1.5" @click="query = ''">查看全部记录</Button>
      </div>

      <nav v-else class="history-list flex min-h-0 flex-1 flex-col gap-2 overflow-auto bg-secondary/45 p-4" aria-label="请求历史" @scroll="scrollList">
        <button
          v-for="entry in entries"
          :key="entry.id"
          type="button"
          data-no-autosave
          :class="[
            'group flex w-full items-center justify-between gap-3.5 rounded-xl border px-4 py-3 text-left transition-all',
            activeId === entry.id
              ? 'active border-primary bg-primary-soft/50 shadow-2xs'
              : 'border-border bg-card hover:border-primary/45 hover:shadow-xs',
          ]"
          :aria-current="activeId === entry.id ? 'true' : undefined"
          @click="openEntry(entry.id)"
        >
          <div class="min-w-0 flex-1">
            <div class="history-item-heading flex items-center gap-2">
              <strong class="truncate text-[13px] font-semibold text-foreground">{{ entry.actionName }}</strong>
              <Badge :variant="historyStatusVariant(entry.status)" class="px-1.5 py-0 text-[10.5px]">{{ historyStatusNames[entry.status] }}</Badge>
              <time :datetime="new Date(entry.createdAt).toISOString()" class="ml-auto shrink-0 text-[11px] tabular-nums text-muted-foreground">
                {{ formatDateTime(entry.createdAt) }}
              </time>
            </div>
            <p class="mb-0 mt-1.5 line-clamp-2 break-words text-xs leading-relaxed text-muted-foreground">{{ entry.preview }}</p>
          </div>
          <span class="shrink-0 -rotate-90 text-muted-foreground/55 transition-all group-hover:translate-x-0.5 group-hover:text-primary" aria-hidden="true">
            <AppIcon name="chevron" :size="15" />
          </span>
        </button>

        <button
          v-if="entries.length < filteredTotal"
          type="button"
          data-no-autosave
          :disabled="loadingMore || searching || refreshing"
          class="mt-0.5 w-full rounded-xl border border-dashed border-border bg-card py-2.5 text-center text-xs font-medium text-muted-foreground transition-colors hover:border-primary hover:bg-primary-soft hover:text-primary"
          @click="loadMore"
        >
          {{ loadingMore ? '正在加载…' : '加载更多' }}
        </button>
      </nav>
    </section>

    <HistoryDetailDialog
      v-model:open="detailOpen"
      :selected="selected"
      :loading="loadingDetail"
      :copied="copied"
      @copy="copyText"
      @close="closeDetail"
    />
  </div>
</template>
