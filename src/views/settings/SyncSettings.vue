<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, toRaw } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import AppIcon from '@/components/AppIcon.vue'
import { command, errorMessage, onEvent } from '@/lib/bridge'
import type { DataInfo, DataOperationResult, SyncPreferences, SyncStatus } from '@/types'

const props = defineProps<{ settingsSaving: boolean; prepare: () => Promise<void> }>()
const emit = defineEmits<{ busy: [value: boolean] }>()
const info = ref<DataInfo | null>(null)
const preferences = ref<SyncPreferences>({ url: '', username: '', password: '', autoSync: false, intervalMinutes: 5, includeApiKeys: false })
const status = ref<SyncStatus>({ busy: false, lastSyncedAt: 0, message: '', error: '' })
const action = ref('')
const message = ref('')
const error = ref('')
const showPassword = ref(false)
const confirmingImport = ref(false)
const saving = ref(false)
const unlisteners: UnlistenFn[] = []
let disposed = false
let refreshRevision = 0
let statusRevision = 0
const busy = computed(() => !!action.value || status.value.busy || saving.value || props.settingsSaving)
const lastSync = computed(() => status.value.lastSyncedAt ? new Date(status.value.lastSyncedAt).toLocaleString('zh-CN', { hour12: false }) : '尚未同步')

function payload(): SyncPreferences { return JSON.parse(JSON.stringify(toRaw(preferences.value))) as SyncPreferences }

async function refresh(): Promise<void> {
  const revision = ++refreshRevision
  const currentStatus = statusRevision
  const result = await command<DataInfo>('get_data_info')
  if (disposed || revision !== refreshRevision) return
  info.value = result
  if (currentStatus === statusRevision) status.value = result.status
}

async function savePreferences(): Promise<void> {
  const draft = payload()
  const saved = await command<SyncPreferences>('save_sync_preferences', { preferences: draft })
  if (!disposed && JSON.stringify(payload()) === JSON.stringify(draft)) preferences.value = saved
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
      await savePreferences()
      const result = await command<DataOperationResult>('sync_webdav')
      message.value = `${result.message}（${result.historyCount} 条）`
      await refresh()
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
      ['data-changed', () => { void refresh().catch((reason) => { if (!disposed) error.value = errorMessage(reason) }) }],
      ['history-changed', () => { void refresh().catch((reason) => { if (!disposed) error.value = errorMessage(reason) }) }],
    ] as const) {
      const unlisten = await onEvent(name, handler)
      if (disposed) { unlisten(); return }
      unlisteners.push(unlisten)
    }
  } catch (reason) { if (!disposed) error.value = errorMessage(reason) }
})

onBeforeUnmount(() => { disposed = true; unlisteners.forEach((unlisten) => unlisten()); emit('busy', false) })
</script>

<template>
  <div class="sync-settings" @input.stop @change.stop @focusout.stop>
    <div v-if="error || status.error" class="notice error-notice" role="alert">{{ error || status.error }}</div>
    <div v-if="message" class="notice sync-success" role="status">{{ message }}</div>
    <section class="settings-card sync-card">
      <div class="section-title"><div><h2>本地备份</h2><span>设置与历史记录打包为 ZIP，可迁移到其他设备</span></div><span class="sync-count">{{ info?.historyCount ?? 0 }} 条记录</span></div>
      <div class="sync-backup-actions">
        <button class="secondary-button" type="button" data-no-autosave :disabled="busy || !info" @click="run('export')"><AppIcon name="download" :size="15" />{{ action === 'export' ? '正在导出…' : '导出 ZIP' }}</button>
        <button class="secondary-button" type="button" data-no-autosave :disabled="busy || !info" @click="confirmingImport = !confirmingImport"><AppIcon name="upload" :size="15" />导入 ZIP</button>
        <label class="sync-checkbox"><input v-model="preferences.includeApiKeys" type="checkbox" :disabled="busy" />包含 API Key</label>
      </div>
      <p class="sync-help">此选项同时用于 ZIP 导出和 WebDAV 同步。未勾选时保留本机已有 Key，新设备需重新填写；勾选后压缩包内包含明文 Key。</p>
      <div v-if="confirmingImport" class="sync-confirm" role="alert">
        <span>将应用备份中的设置，并合并历史记录。导入前自动备份当前数据，保留最近 5 份；自动备份包含 API Key，仅保存在本机。</span>
        <div class="sync-buttons"><button class="secondary-button small-button" type="button" data-no-autosave :disabled="busy" @click="confirmingImport = false">取消</button><button class="primary-button" type="button" data-no-autosave :disabled="busy" @click="run('import')">{{ action === 'import' ? '正在导入…' : '选择 ZIP 并导入' }}</button></div>
      </div>
      <div class="sync-directory"><span>数据目录</span><code :title="info?.directory">{{ info?.directory ?? '正在加载…' }}</code><button class="secondary-button small-button" type="button" data-no-autosave :disabled="!info" @click="openDirectory">打开目录</button></div>
    </section>

    <section class="settings-card sync-card">
      <div class="section-title"><div><h2>WebDAV 多设备同步</h2><span>各设备使用相同目录，历史合并去重，设置采用较新版本</span></div></div>
      <fieldset class="sync-fields" :disabled="busy || !info">
        <label class="field field-wide"><span>WebDAV 目录地址</span><input v-model.trim="preferences.url" type="url" placeholder="https://dav.example.com/CitrI/" spellcheck="false" /><small>同步文件为目录下的 <code>citri-sync.zip</code>；首次同步尝试创建目录，上级目录需已存在</small></label>
        <label class="field"><span>用户名</span><input v-model.trim="preferences.username" autocomplete="off" spellcheck="false" /></label>
        <label class="field"><span>密码 / 应用专用密码</span><div class="input-with-button"><input v-model="preferences.password" :type="showPassword ? 'text' : 'password'" autocomplete="new-password" spellcheck="false" /><button type="button" data-no-autosave :title="showPassword ? '隐藏密码' : '显示密码'" @click="showPassword = !showPassword"><AppIcon :name="showPassword ? 'eyeoff' : 'eye'" :size="15" /></button></div></label>
        <label class="sync-checkbox"><input v-model="preferences.autoSync" type="checkbox" />自动同步</label>
        <label class="field sync-interval"><span>每隔</span><input v-model.number="preferences.intervalMinutes" type="number" min="1" max="1440" /><span>分钟</span></label>
      </fieldset>
      <div class="sync-webdav-actions">
        <div class="sync-buttons"><button class="secondary-button" type="button" data-no-autosave :disabled="busy || !info" @click="save">{{ saving ? '正在保存…' : '保存同步设置' }}</button><button class="secondary-button" type="button" data-no-autosave :disabled="busy || !preferences.url" @click="run('test')">{{ action === 'test' ? '测试中…' : '测试连接' }}</button><button class="primary-button" type="button" data-no-autosave :disabled="busy || !preferences.url" @click="run('sync')"><AppIcon name="refresh" :size="14" />{{ action === 'sync' || status.busy ? '正在同步…' : '立即同步' }}</button></div>
        <span class="sync-last" aria-live="polite">{{ status.busy ? '正在同步…' : lastSync }}</span>
      </div>
      <p class="sync-help">首次连接已有云端数据时采用云端设置。清空历史也会同步到其他设备；WebDAV 账号密码与窗口尺寸仅保存在本机。</p>
    </section>
  </div>
</template>

<style scoped>
.sync-settings { display: flex; flex-direction: column; gap: 14px; }
.sync-card { width: 100%; padding: 18px; }
.sync-settings > .notice { width: 100%; margin-bottom: 0; }
.sync-success { color: var(--success); border-color: color-mix(in srgb, var(--success), transparent 60%); background: color-mix(in srgb, var(--success), transparent 93%); overflow-wrap: anywhere; }
.sync-count, .sync-last { color: var(--muted-foreground); font-size: calc(11px * var(--font-scale)); }
.sync-backup-actions, .sync-buttons { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
.sync-checkbox { display: inline-flex; align-items: center; gap: 7px; font-size: calc(12px * var(--font-scale)); }
.sync-checkbox input { accent-color: var(--primary); }
.sync-help { margin: 12px 0 0; color: var(--muted-foreground); font-size: calc(11px * var(--font-scale)); line-height: 1.6; }
.sync-directory { display: flex; align-items: center; gap: 10px; padding-top: 12px; margin-top: 12px; border-top: 1px solid var(--border); font-size: calc(11px * var(--font-scale)); color: var(--muted-foreground); }
.sync-directory > span { flex-shrink: 0; }
.sync-directory code { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: calc(10px * var(--font-scale)); }
.sync-directory button { flex-shrink: 0; }
.sync-fields { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; padding: 0; margin: 0; border: 0; }
.sync-interval { flex-direction: row; align-items: center; justify-content: flex-end; gap: 7px; }
.sync-interval input[type=number] { width: 78px; }
.sync-webdav-actions { display: flex; align-items: center; flex-wrap: wrap; justify-content: space-between; gap: 12px; margin-top: 18px; }
.sync-confirm { display: flex; flex-direction: column; gap: 10px; padding: 12px; margin-top: 12px; border: 1px solid var(--border); border-radius: 7px; background: var(--secondary); font-size: calc(12px * var(--font-scale)); line-height: 1.5; }
@media (max-width: 820px) { .sync-card { padding: 15px; } }
</style>
