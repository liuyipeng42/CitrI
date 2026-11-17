<script setup lang="ts">
import { computed, ref } from 'vue'
import AppIcon from '@/components/AppIcon.vue'
import Button from '@/components/ui/Button.vue'
import Select, { type SelectOption } from '@/components/ui/Select.vue'
import Switch from '@/components/ui/Switch.vue'
import { defaultActionPrompts } from './defaultPrompts'
import { useDragReorder } from './useDragReorder'
import type { ActionItem, Settings } from '@/types'

const settings = defineModel<Settings>({ required: true })
const sectionEl = ref<HTMLElement | null>(null)
const listEl = ref<HTMLElement | null>(null)
const expandedId = ref<string | null>(null)
const actionsRef = computed({
  get: () => settings.value.actions,
  set: (value) => { settings.value.actions = value },
})

const { draggingId, startCardDrag, itemWrapperStyle, itemWrapperClass, moveBy } = useDragReorder(
  actionsRef,
  listEl,
  () => sectionEl.value?.dispatchEvent(new Event('change', { bubbles: true })),
)

const iconOptions: SelectOption[] = [
  { value: 'copy', label: '双页图标' },
  { value: 'search', label: '放大镜图标' },
  { value: 'languages', label: '语言图标' },
  { value: 'align-left', label: '文本图标' },
  { value: 'message-circle-question', label: '问答气泡图标' },
  { value: 'sparkles', label: '星光图标' },
]

function onHandleKeydown(event: KeyboardEvent, index: number): void {
  if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
  event.preventDefault()
  moveBy(index, event.key === 'ArrowUp' ? -1 : 1)
}

function addAction(): void {
  if (settings.value.actions.length >= 20) return
  const id = `custom-${Date.now()}-${settings.value.actions.length + 1}`
  settings.value.actions.push({
    id,
    name: '自定义', icon: 'sparkles', enabled: true, builtIn: false,
    prompt: '请处理下面的文本：\n\n{{text}}',
  })
  expandedId.value = id
}

function removeAction(index: number): void {
  const removed = settings.value.actions[index]
  if (!removed || removed.builtIn) return
  settings.value.actions.splice(index, 1)
  if (expandedId.value === removed.id) expandedId.value = null
}

function toggleExpand(id: string): void {
  expandedId.value = expandedId.value === id ? null : id
}

function canRestoreDefaultPrompt(action: ActionItem): boolean {
  const fallback = defaultActionPrompts[action.id]
  return Boolean(action.builtIn && fallback && action.prompt !== fallback)
}

function restoreDefaultPrompt(action: ActionItem): void {
  const fallback = defaultActionPrompts[action.id]
  if (fallback) action.prompt = fallback
}
</script>

<template>
  <section ref="sectionEl" class="mx-auto max-w-[880px] rounded-2xl border border-border bg-card p-6 shadow-xs">
    <div class="mb-4 flex items-center justify-between gap-3">
      <div>
        <h2 class="m-0 text-[15px] font-semibold tracking-tight text-foreground">动作与提示词</h2>
        <span class="mt-1 block text-[11.5px] text-muted-foreground">按住卡片或左侧手柄拖动调整顺序，点击任意动作展开编辑 Prompt，使用 <code v-pre class="rounded bg-secondary px-1 py-0.5 text-primary">{{text}}</code> 插入选中文本</span>
      </div>
      <Button variant="secondary" :disabled="settings.actions.length >= 20" @click="addAction">
        <AppIcon name="plus" :size="14" />添加自定义动作
      </Button>
    </div>

    <div ref="listEl" class="flex flex-col gap-2.5">
      <div
        v-for="(action, index) in settings.actions"
        :key="action.id"
        :style="itemWrapperStyle(action.id, index)"
        :class="['relative will-change-transform motion-reduce:transition-none', itemWrapperClass(action.id)]"
        @pointerdown="startCardDrag($event, action.id, index)"
      >
        <article
          :class="[
            'flex flex-col gap-2.5 rounded-xl border p-3 transition-[transform,box-shadow,border-color,background-color] duration-200 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none',
            draggingId === action.id
              ? 'scale-[1.02] cursor-grabbing border-primary/70 bg-card shadow-[0_20px_42px_-10px_rgba(15,18,24,0.24),0_8px_18px_-6px_rgba(15,18,24,0.12)] ring-2 ring-primary/20 dark:shadow-[0_22px_48px_-8px_rgba(0,0,0,0.65)]'
              : expandedId === action.id
                ? 'border-primary/50 bg-card shadow-xs'
                : 'border-border bg-secondary/70 hover:border-primary/35',
            !action.enabled && draggingId !== action.id ? 'bg-secondary/45' : '',
          ]"
        >
          <div class="grid grid-cols-[20px_38px_30px_minmax(96px,1fr)_140px_auto] items-center gap-2.5 max-[820px]:grid-cols-[20px_38px_28px_minmax(80px,1fr)_114px_auto]">
            <button
              type="button"
              data-no-autosave
              data-drag-handle
              title="拖动调整顺序（也可用方向键 ↑/↓）"
              aria-label="拖动排序"
              class="grid h-7 w-5 cursor-grab place-items-center rounded border-0 bg-transparent p-0 text-muted-foreground/55 transition-colors hover:text-foreground active:cursor-grabbing"
              @keydown="onHandleKeydown($event, index)"
            >
              <AppIcon name="grip" :size="15" />
            </button>
            <Switch v-model="action.enabled" />
            <span :class="['grid h-[30px] w-[30px] place-items-center rounded-lg bg-primary-soft text-primary transition-opacity', !action.enabled ? 'opacity-55' : '']">
              <AppIcon :name="action.icon" :size="15" />
            </span>
            <input
              v-model="action.name"
              maxlength="40"
              placeholder="动作名称"
              :class="['h-8 w-full rounded-lg border border-border bg-card px-2.5 text-xs text-foreground shadow-2xs transition-opacity focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20', !action.enabled ? 'opacity-65' : '']"
            />
            <Select
              v-model="action.icon"
              :options="iconOptions"
              aria-label="按钮图标（仅改变外观）"
              title="按钮图标：只改变外观，不影响动作功能"
            />
            <div class="flex items-center gap-1">
              <Button
                v-if="action.id !== 'copy'"
                size="icon"
                variant="outline"
                data-no-autosave
                :title="expandedId === action.id ? '收起配置' : '编辑配置'"
                :aria-expanded="expandedId === action.id"
                :class="expandedId === action.id ? 'border-primary/50 bg-primary-soft text-primary' : 'bg-card'"
                @click="toggleExpand(action.id)"
              >
                <AppIcon name="chevron" :size="14" :class="expandedId === action.id ? 'rotate-180 transition-transform' : 'transition-transform'" />
              </Button>
              <Button
                v-if="!action.builtIn"
                size="icon"
                variant="outline"
                title="删除"
                aria-label="删除"
                class="bg-card hover:border-destructive/50 hover:bg-destructive/10 hover:text-destructive"
                @click="removeAction(index)"
              >
                <AppIcon name="trash" :size="14" />
              </Button>
            </div>
          </div>

          <div v-if="action.id === 'copy'" class="pl-[108px] text-[11.5px] text-muted-foreground">内置复制动作，选中后直接写入剪贴板</div>
          <div v-else-if="expandedId === action.id" class="animate-panel-in">
            <label v-if="action.id === 'search'" class="grid grid-cols-[70px_minmax(0,1fr)] items-center gap-2 pt-1 text-xs">
              <span class="font-medium text-foreground">搜索 URL</span>
              <input
                v-model.trim="settings.searchUrl"
                spellcheck="false"
                class="h-8 w-full rounded-lg border border-border bg-secondary/50 px-2.5 text-xs focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
              <small class="col-start-2 text-[11px] text-muted-foreground">使用 <code v-pre class="rounded bg-secondary px-1 py-0.5 text-primary">{{queryString}}</code> 插入 URL 编码后的选中文本</small>
            </label>
            <div v-else class="flex flex-col gap-2">
              <div class="flex items-center justify-between gap-2 pl-1">
                <span class="text-[11px] text-muted-foreground">支持 Markdown 输出，使用 <code v-pre class="rounded bg-secondary px-1 py-0.5 text-primary">{{text}}</code> 插入选中文本</span>
                <Button
                  v-if="canRestoreDefaultPrompt(action)"
                  size="sm"
                  variant="ghost"
                  class="h-6.5 px-2 text-[11px] text-primary hover:bg-primary-soft hover:text-primary"
                  @click="restoreDefaultPrompt(action)"
                >
                  <AppIcon name="refresh" :size="12" />恢复默认 Prompt
                </Button>
              </div>
              <textarea
                v-model="action.prompt"
                rows="5"
                aria-label="AI 动作提示词"
                placeholder="告诉 AI 如何处理选中文本，例如：纠正语法错误并说明修改原因。"
                class="min-h-24 w-full resize-y rounded-lg border border-border bg-secondary/50 px-3 py-2 text-xs leading-relaxed text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
              />
            </div>
          </div>
          <button
            v-else
            type="button"
            data-no-autosave
            class="w-full truncate border-0 bg-transparent py-0.5 pl-[108px] text-left text-[11.5px] text-muted-foreground hover:text-foreground"
            @click="toggleExpand(action.id)"
          >
            {{ action.id === 'search' ? settings.searchUrl : (action.prompt || '点击填写 AI 提示词…') }}
          </button>
        </article>
      </div>
    </div>
  </section>
</template>
