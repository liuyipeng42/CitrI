<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import AppIcon from '@/components/AppIcon.vue'
import PasswordInput from '@/components/PasswordInput.vue'
import Badge from '@/components/ui/Badge.vue'
import Button from '@/components/ui/Button.vue'
import Slider from '@/components/ui/Slider.vue'
import ExtraParametersCard from './ExtraParametersCard.vue'
import { parametersFromBody, readExtraBody, type ExtraParameter } from './extraParameters'
import { command, errorMessage } from '@/lib/bridge'
import { cloneRaw } from '@/lib/utils'
import type { ApiProfile, Settings } from '@/types'

type StatusKind = 'normal' | 'success' | 'error'
const settings = defineModel<Settings>({ required: true })
const emit = defineEmits<{
  status: [message: string, kind?: StatusKind]
  'draft-change': []
}>()

const testing = ref(false)
const extraParameters = ref<ExtraParameter[]>([])
const activeProfile = computed(() => settings.value.apiProfiles.find((profile) => profile.id === settings.value.activeApiProfileId) ?? null)

function commit(): ApiProfile {
  const profile = activeProfile.value
  if (!profile) throw new Error('没有可用的 API 配置')
  profile.name = profile.name.trim() || '未命名 API'
  profile.apiKey = profile.apiKey.trim()
  const rawMaxTokens = profile.maxTokens as unknown
  const maxTokens = rawMaxTokens === '' ? Number.NaN : Number(rawMaxTokens)
  profile.maxTokens = Number.isFinite(maxTokens) ? Math.min(128000, Math.max(1, Math.round(maxTokens))) : 2048
  const temperature = Number(profile.temperature)
  profile.temperature = Number.isFinite(temperature) ? Math.min(2, Math.max(0, Math.round(temperature * 10) / 10)) : 0.3
  profile.extraBody = readExtraBody(extraParameters.value)
  return profile
}

function reload(): void {
  extraParameters.value = parametersFromBody(activeProfile.value?.extraBody ?? {})
}

function selectProfile(id: string): void {
  if (id === settings.value.activeApiProfileId) return
  try {
    commit()
    settings.value.activeApiProfileId = id
    reload()
    emit('status', '已切换')
  } catch (error) { emit('status', errorMessage(error), 'error') }
}

function addProfile(): void {
  if (settings.value.apiProfiles.length >= 20) return emit('status', '最多可添加 20 个 API 配置', 'error')
  try {
    const previous = commit()
    const profile: ApiProfile = {
      id: `api-${Date.now()}-${settings.value.apiProfiles.length + 1}`,
      name: `API ${settings.value.apiProfiles.length + 1}`,
      apiUrl: 'https://api.openai.com/v1', apiKey: '', model: 'gpt-4.1-mini',
      systemPrompt: previous.systemPrompt || '你是一个准确、简洁、可靠的划词助手。',
      temperature: 0.3, maxTokens: 2048, extraBody: {},
    }
    settings.value.apiProfiles.push(profile)
    settings.value.activeApiProfileId = profile.id
    reload()
    emit('status', '已添加 API 配置')
  } catch (error) { emit('status', errorMessage(error), 'error') }
}

function deleteProfile(): void {
  if (settings.value.apiProfiles.length <= 1) return
  const index = settings.value.apiProfiles.findIndex((profile) => profile.id === settings.value.activeApiProfileId)
  settings.value.apiProfiles.splice(index, 1)
  const next = settings.value.apiProfiles[Math.min(Math.max(index, 0), settings.value.apiProfiles.length - 1)]!
  settings.value.activeApiProfileId = next.id
  reload()
  emit('status', '已删除')
}

async function testApi(): Promise<void> {
  testing.value = true
  try {
    const result = await command<string>('test_api', { profile: cloneRaw(commit()) })
    emit('status', `连接成功：${result.slice(0, 40)}`, 'success')
  } catch (error) { emit('status', errorMessage(error), 'error') }
  finally { testing.value = false }
}

onMounted(reload)
defineExpose({ commit, reload })
</script>

<template>
  <div class="mx-auto flex max-w-[880px] flex-col gap-4">
    <section class="rounded-2xl border border-border bg-card p-5 shadow-xs">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <div class="flex items-center gap-2">
            <h2 class="m-0 text-[15px] font-semibold tracking-tight text-foreground">API 与模型配置</h2>
            <Badge variant="muted">{{ settings.apiProfiles.length }}/20</Badge>
          </div>
          <span class="mt-1 block text-[11.5px] text-muted-foreground">当前激活的配置将用于所有 AI 动作，可随时在上方快速切换</span>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <Button variant="secondary" :disabled="testing" data-no-autosave @click="testApi">
            <span v-if="testing" class="spinner h-3.5 w-3.5" />
            <AppIcon v-else name="sparkles" :size="14" />
            {{ testing ? '测试中…' : '测试当前 API' }}
          </Button>
          <Button variant="default" @click="addProfile">
            <AppIcon name="plus" :size="14" />添加配置
          </Button>
        </div>
      </div>

      <div class="mt-4 flex flex-wrap gap-2 rounded-xl border border-border/80 bg-secondary/55 p-2">
        <button
          v-for="profile in settings.apiProfiles"
          :key="profile.id"
          type="button"
          :class="[
            'flex min-w-0 max-w-full items-center gap-2 rounded-lg border px-3 py-1.5 text-left transition-all',
            profile.id === settings.activeApiProfileId
              ? 'border-primary/45 bg-card text-foreground shadow-xs ring-1 ring-primary/15'
              : 'border-transparent bg-transparent text-muted-foreground hover:border-border/70 hover:bg-card/80 hover:text-foreground',
          ]"
          @click="selectProfile(profile.id)"
        >
          <span :class="['h-2 w-2 shrink-0 rounded-full transition-colors', profile.id === settings.activeApiProfileId ? 'bg-primary' : 'bg-muted-foreground/35']" />
          <span class="max-w-[160px] truncate text-xs font-semibold">{{ profile.name }}</span>
          <span
            v-if="profile.model"
            :class="[
              'max-w-[150px] truncate rounded px-1.5 py-0.5 font-mono text-[10.5px]',
              profile.id === settings.activeApiProfileId ? 'bg-primary-soft text-primary' : 'bg-muted/85 text-muted-foreground',
            ]"
          >{{ profile.model }}</span>
        </button>
      </div>
    </section>

    <template v-if="activeProfile">
      <section class="rounded-2xl border border-border bg-card p-6 shadow-xs">
        <div class="mb-4 flex flex-wrap items-center justify-between gap-3 border-b border-border/70 pb-3.5">
          <div class="flex items-center gap-2">
            <h3 class="m-0 text-[14px] font-semibold tracking-tight text-foreground">连接与凭证</h3>
            <Badge :variant="activeProfile.apiKey.trim() ? 'success' : 'secondary'">{{ activeProfile.apiKey.trim() ? '已填 Key' : '未填 Key' }}</Badge>
          </div>
          <Button
            variant="outline"
            size="sm"
            title="删除当前 API"
            :disabled="settings.apiProfiles.length <= 1"
            class="h-8 text-xs hover:border-destructive/50 hover:bg-destructive/10 hover:text-destructive"
            @click="deleteProfile"
          >
            <AppIcon name="trash" :size="13" />删除配置
          </Button>
        </div>

        <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <label class="flex min-w-0 flex-col gap-1.5 text-xs">
            <span class="font-semibold text-foreground">配置名称</span>
            <input
              v-model="activeProfile.name"
              maxlength="50"
              placeholder="例如：OpenAI / DeepSeek"
              class="h-9 w-full rounded-lg border border-border bg-background px-3 text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
            />
          </label>

          <label class="flex min-w-0 flex-col gap-1.5 text-xs">
            <span class="font-semibold text-foreground">模型</span>
            <input
              v-model.trim="activeProfile.model"
              placeholder="gpt-4.1-mini"
              spellcheck="false"
              class="h-9 w-full rounded-lg border border-border bg-background px-3 font-mono text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
            />
          </label>

          <label class="col-span-full flex min-w-0 flex-col gap-1.5 text-xs">
            <span class="font-semibold text-foreground">API URL</span>
            <input
              v-model.trim="activeProfile.apiUrl"
              type="url"
              placeholder="https://api.openai.com/v1"
              spellcheck="false"
              class="h-9 w-full rounded-lg border border-border bg-background px-3 text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
            />
            <small class="text-[11px] text-muted-foreground">可填写基础地址，或完整的 <code class="rounded bg-secondary px-1 py-0.5 text-primary">/chat/completions</code> 地址</small>
          </label>

          <label class="col-span-full flex min-w-0 flex-col gap-1.5 text-xs">
            <span class="font-semibold text-foreground">API Key</span>
            <PasswordInput
              v-model="activeProfile.apiKey"
              placeholder="sk-..."
              show-title="显示 API Key"
              hide-title="隐藏 API Key"
            />
            <small class="text-[11px] text-muted-foreground">仅由 Rust 后端读取并保存在本机，不会发送给工具条和结果窗口</small>
          </label>
        </div>
      </section>

      <section class="rounded-2xl border border-border bg-card p-6 shadow-xs">
        <div class="mb-4 border-b border-border/70 pb-3.5">
          <h3 class="m-0 text-[14px] font-semibold tracking-tight text-foreground">生成参数与系统 Prompt</h3>
          <span class="mt-1 block text-[11.5px] text-muted-foreground">控制模型输出随机性、长度上限以及默认系统角色设定</span>
        </div>

        <div class="grid grid-cols-1 items-end gap-5 sm:grid-cols-[minmax(0,1fr)_220px]">
          <div class="flex min-w-0 flex-col gap-2 text-xs">
            <div class="flex items-center justify-between gap-2">
              <div class="flex items-center gap-2">
                <span class="font-semibold text-foreground">温度</span>
                <Badge variant="default">{{ activeProfile.temperature.toFixed(1) }}</Badge>
              </div>
              <span class="text-[11px] text-muted-foreground">
                {{ activeProfile.temperature <= 0.4 ? '严谨稳定' : activeProfile.temperature <= 1.0 ? '均衡自然' : '更有创意' }}
              </span>
            </div>
            <Slider v-model="activeProfile.temperature" :min="0" :max="2" :step="0.1" />
          </div>

          <label class="flex min-w-0 flex-col gap-1.5 text-xs">
            <span class="font-semibold text-foreground">最大输出 Token</span>
            <input
              v-model.number="activeProfile.maxTokens"
              type="number"
              min="1"
              max="128000"
              class="h-9 w-full rounded-lg border border-border bg-background px-3 text-xs tabular-nums text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
            />
          </label>
        </div>

        <label class="mt-4 flex min-w-0 flex-col gap-1.5 text-xs">
          <span class="font-semibold text-foreground">系统 Prompt</span>
          <textarea
            v-model="activeProfile.systemPrompt"
            rows="3"
            class="w-full resize-y rounded-lg border border-border bg-background px-3 py-2.5 text-xs leading-relaxed text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
        </label>
      </section>

      <ExtraParametersCard v-model="extraParameters" @draft-change="emit('draft-change')" />
    </template>
  </div>
</template>
