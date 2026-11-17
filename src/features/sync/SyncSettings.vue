<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useAppEvents } from '@/lib/events'
import AppIcon from '@/components/AppIcon.vue'
import PasswordInput from '@/components/PasswordInput.vue'
import Badge from '@/components/ui/Badge.vue'
import Button from '@/components/ui/Button.vue'
import { command, errorMessage } from '@/lib/bridge'
import { cloneRaw, formatDateTime } from '@/lib/utils'
import type { DataInfo, DataOperationResult, SyncPreferences, SyncStatus } from '@/types'

const props = defineProps<{ settingsSaving: boolean; prepare: () => Promise<void> }>()
const emit = defineEmits<{ busy: [value: boolean] }>()
const info = ref<DataInfo | null>(null)
const preferences = ref<SyncPreferences>({ url: '', username: '', password: '', includeApiKeys: false })
const status = ref<SyncStatus>({ busy: false, lastSyncedAt: 0, message: '', error: '' })
const action = ref('')
const message = ref('')
const error = ref('')
const confirmingImport = ref(false)
const saving = ref(false)
const listenEvent = useAppEvents()
let disposed = false
let refreshRevision = 0
let statusRevision = 0
const busy = computed(() => !!action.value || status.value.busy || saving.value || props.settingsSaving)
const lastSync = computed(() => status.value.lastSyncedAt ? formatDateTime(status.value.lastSyncedAt) : '尚未同步')

function payload(): SyncPreferences { return cloneRaw(preferences.value) }

async function refresh(): Promise<void> {
  const revision = ++refreshRevision
  const currentStatus = statusRevision
  try {
    const result = await command<DataInfo>('get_data_info')
    if (disposed || revision !== refreshRevision) return
    info.value = result
    if (currentStatus === statusRevision) status.value = result.status
  } catch (reason) {
    if (!disposed && revision === refreshRevision) error.value = errorMessage(reason)
  }
}

async function savePreferences(): Promise<void> {
  const draft = payload()
  const snapshot = JSON.stringify(draft)
  const saved = await command<SyncPreferences>('save_sync_preferences', { preferences: draft })
  if (!disposed && JSON.stringify(cloneRaw(preferences.value)) === snapshot) preferences.value = saved
}

async function save(): Promise<void> {
  saving.value = true
  error.value = ''
  try { await savePreferences(); message.value = '同步设置已保存' }
  catch (reason) { error.value = errorMessage(reason) }
  finally { saving.value = false }
}

async function run(name: 'export' | 'import' | 'test' | 'sync'): Promise<void> {
  if (busy.value) return
  action.value = name
  emit('busy', true)
  error.value = ''
  message.value = ''
  try {
    await props.prepare()
    if (name === 'export') {
      const result = await command<DataOperationResult | null>('export_data', { includeApiKeys: preferences.value.includeApiKeys })
      if (result) message.value = `${result.message}：${result.path}`
    } else if (name === 'import') {
      const result = await command<DataOperationResult | null>('import_data')
      confirmingImport.value = false
      if (result) { message.value = `${result.message}（${result.historyCount} 条）`; await refresh() }
    } else if (name === 'test') {
      message.value = await command<string>('test_webdav', { preferences: payload() })
    } else {
      const draft = payload()
      const snapshot = JSON.stringify(draft)
      const result = await command<DataOperationResult>('sync_webdav', { preferences: draft })
      message.value = `${result.message}（${result.historyCount} 条）`
      await refresh()
      if (!disposed && info.value && JSON.stringify(cloneRaw(preferences.value)) === snapshot) {
        preferences.value = info.value.preferences
      }
    }
  } catch (reason) { error.value = errorMessage(reason) }
  finally { action.value = ''; emit('busy', false) }
}

async function openDirectory(): Promise<void> {
  try { await command('open_data_directory') }
  catch (reason) { error.value = errorMessage(reason) }
}

onMounted(async () => {
  try {
    const result = await command<DataInfo>('get_data_info')
    if (disposed) return
    info.value = result
    preferences.value = result.preferences
    status.value = result.status
    for (const [name, handler] of [
      ['sync-status', (event: { payload: unknown }) => { statusRevision += 1; status.value = event.payload as SyncStatus }],
      ['data-changed', () => { void refresh() }],
      ['history-changed', () => { void refresh() }],
    ] as const) {
      if (!await listenEvent(name, handler)) return
    }
  } catch (reason) { if (!disposed) error.value = errorMessage(reason) }
})

onBeforeUnmount(() => { disposed = true; emit('busy', false) })
</script>

<template>
  <div class="sync-settings mx-auto flex max-w-[880px] flex-col gap-4" @input.stop @change.stop @focusout.stop>
    <div v-if="error || status.error" class="notice error-notice animate-pop-in rounded-xl border border-destructive/45 bg-destructive/10 px-3.5 py-2.5 text-xs text-destructive" role="alert">{{ error || status.error }}</div>
    <div v-if="message" class="animate-pop-in rounded-xl border border-success/45 bg-success/10 px-3.5 py-2.5 text-xs text-success break-words" role="status">{{ message }}</div>

    <section class="rounded-2xl border border-border bg-card p-6 shadow-xs">
      <div class="mb-4 flex items-start justify-between gap-3">
        <div>
          <h2 class="m-0 text-[15px] font-semibold tracking-tight text-foreground">本地数据与备份</h2>
          <span class="mt-1 block text-[11.5px] text-muted-foreground">将设置与历史记录打包为 ZIP，便于本地存档或迁移到其他设备</span>
        </div>
        <Badge variant="secondary">{{ info?.historyCount ?? 0 }} 条记录</Badge>
      </div>

      <div class="sync-backup-actions flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border bg-secondary/60 px-3.5 py-3">
        <div class="flex flex-wrap items-center gap-2">
          <Button variant="secondary" data-no-autosave :disabled="busy || !info" @click="run('export')">
            <AppIcon name="download" :size="15" />{{ action === 'export' ? '正在导出…' : '导出 ZIP' }}
          </Button>
          <Button variant="secondary" data-no-autosave :disabled="busy || !info" @click="confirmingImport = !confirmingImport">
            <AppIcon name="upload" :size="15" />导入 ZIP
          </Button>
        </div>

        <label class="inline-flex cursor-pointer select-none items-center gap-2.5 text-xs font-medium text-foreground">
          <span>包含 API Key</span>
          <span class="relative inline-flex h-[21px] w-[38px] items-center">
            <input v-model="preferences.includeApiKeys" type="checkbox" :disabled="busy" class="peer sr-only" />
            <span class="h-full w-full rounded-full bg-muted-foreground/30 transition-colors peer-checked:bg-primary peer-focus-visible:ring-2 peer-focus-visible:ring-primary/35 peer-disabled:opacity-50" />
            <span class="pointer-events-none absolute left-[3px] h-[15px] w-[15px] rounded-full bg-white shadow-sm transition-transform peer-checked:translate-x-[17px]" />
          </span>
        </label>
      </div>

      <div v-if="confirmingImport" class="sync-confirm animate-pop-in mt-3 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-primary/40 bg-primary-soft px-3.5 py-3 text-xs leading-relaxed" role="alert">
        <span class="min-w-[260px] flex-1 text-foreground">将应用备份中的设置，并合并历史记录。导入前自动备份当前数据，保留最近 5 份；自动备份包含 API Key，仅保存在本机。</span>
        <div class="flex items-center gap-2">
          <Button size="sm" variant="secondary" data-no-autosave :disabled="busy" @click="confirmingImport = false">取消</Button>
          <Button size="sm" variant="default" data-no-autosave :disabled="busy" @click="run('import')">{{ action === 'import' ? '正在导入…' : '选择 ZIP 并导入' }}</Button>
        </div>
      </div>

      <div class="mt-3 flex items-center gap-2.5 rounded-xl border border-border bg-secondary px-3.5 py-2.5 text-xs text-muted-foreground">
        <span class="shrink-0 font-semibold text-foreground">数据目录</span>
        <code :title="info?.directory" class="min-w-0 flex-1 truncate rounded-md border border-border bg-card px-2 py-1 text-[11px] text-muted-foreground">{{ info?.directory ?? '正在加载…' }}</code>
        <Button size="sm" variant="secondary" data-no-autosave :disabled="!info" @click="openDirectory">打开目录</Button>
      </div>

      <p class="mt-3 mb-0 text-[11px] leading-relaxed text-muted-foreground">「包含 API Key」同时作用于 ZIP 导出与 WebDAV 同步：未开启时保留本机已有 Key，新设备需重新填写；开启后压缩包内包含明文 Key。</p>
    </section>

    <section class="rounded-2xl border border-border bg-card p-6 shadow-xs">
      <div class="mb-4 flex items-start justify-between gap-3">
        <div>
          <h2 class="m-0 text-[15px] font-semibold tracking-tight text-foreground">WebDAV 多设备同步</h2>
          <span class="mt-1 block text-[11.5px] text-muted-foreground">各设备连接同一目录，历史记录自动合并去重，设置采用较新版本</span>
        </div>
        <Badge :variant="status.busy ? 'default' : 'secondary'" aria-live="polite">
          <span :class="['h-1.5 w-1.5 rounded-full', status.busy ? 'bg-primary' : status.lastSyncedAt > 0 ? 'bg-success' : 'bg-muted-foreground/60']" />
          {{ status.busy ? '正在同步…' : status.lastSyncedAt ? `上次同步 ${lastSync}` : lastSync }}
        </Badge>
      </div>

      <fieldset class="grid grid-cols-2 gap-4 border-0 p-0" :disabled="busy || !info">
        <label class="col-span-full flex min-w-0 flex-col gap-1.5 text-xs">
          <span class="font-semibold text-foreground">WebDAV 目录地址</span>
          <input
            v-model.trim="preferences.url"
            type="url"
            placeholder="https://dav.example.com/CitrI/"
            spellcheck="false"
            class="h-8.5 w-full rounded-lg border border-border bg-background px-2.5 text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
          <small class="text-[11px] text-muted-foreground">同步文件为目录下的 <code class="rounded bg-secondary px-1 py-0.5 text-primary">citri-sync.zip</code>；首次同步尝试创建目录，上级目录需已存在</small>
        </label>

        <label class="flex min-w-0 flex-col gap-1.5 text-xs">
          <span class="font-semibold text-foreground">用户名</span>
          <input
            v-model.trim="preferences.username"
            placeholder="选填，视服务端要求"
            autocomplete="off"
            spellcheck="false"
            class="h-8.5 w-full rounded-lg border border-border bg-background px-2.5 text-xs text-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
          />
        </label>

        <label class="flex min-w-0 flex-col gap-1.5 text-xs">
          <span class="font-semibold text-foreground">密码 / 应用专用密码</span>
          <PasswordInput
            v-model="preferences.password"
            autocomplete="new-password"
            show-title="显示密码"
            hide-title="隐藏密码"
            input-class="h-8.5 px-2.5"
          />
        </label>
      </fieldset>

      <div class="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-border pt-4">
        <div class="flex flex-wrap items-center gap-2">
          <Button variant="secondary" data-no-autosave :disabled="busy || !info" @click="save">{{ saving ? '正在保存…' : '保存同步设置' }}</Button>
          <Button variant="secondary" data-no-autosave :disabled="busy || !preferences.url" @click="run('test')">{{ action === 'test' ? '测试中…' : '测试连接' }}</Button>
        </div>
        <Button variant="default" data-no-autosave :disabled="busy || !preferences.url" @click="run('sync')">
          <AppIcon name="refresh" :size="14" :class="action === 'sync' || status.busy ? 'animate-spin' : ''" />{{ action === 'sync' || status.busy ? '正在同步…' : '立即同步' }}
        </Button>
      </div>

      <p class="mt-3 mb-0 text-[11px] leading-relaxed text-muted-foreground">配置并保存 WebDAV 地址后，启动应用时以及本地设置、历史记录发生改动后将自动同步；首次连接已有云端数据时采用云端设置。</p>
    </section>
  </div>
</template>
