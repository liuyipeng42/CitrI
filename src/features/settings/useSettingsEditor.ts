import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useAppEvents } from '@/lib/events'
import type ApiSettings from './api/ApiSettings.vue'
import { command, errorMessage } from '@/lib/bridge'
import { applyAppearance } from '@/lib/theme'
import { cloneRaw } from '@/lib/utils'
import type { PlatformInfo, Settings } from '@/types'

export function useSettingsEditor() {
  const settings = ref<Settings | null>(null)
  const status = ref('')
  const statusKind = ref<'normal' | 'success' | 'error'>('normal')
  const serviceError = ref('')
  const platformNotice = ref('')
  const saving = ref(false)
  const hasUnsavedDraft = ref(false)
  const filterListText = ref('')
  const apiSettings = ref<InstanceType<typeof ApiSettings> | null>(null)
  let statusTimer: number | undefined
  const listenEvent = useAppEvents()
  let editingSignaled = false
  let editRevision = 0
  let saveRequested = false
  const editedFields = new WeakSet<Element>()

  function setStatus(message: string, kind: 'normal' | 'success' | 'error' = 'normal'): void {
    window.clearTimeout(statusTimer)
    status.value = message
    statusKind.value = kind
    if (kind === 'success') statusTimer = window.setTimeout(() => (status.value = ''), 2600)
  }

  function requestSave(): void {
    noteLocalDraftChange()
    saveRequested = true
    void flushSaves()
  }

  function signalEditing(editing: boolean): void {
    if (editingSignaled === editing) return
    editingSignaled = editing
    void command('set_sync_editing', { editing }).catch(() => {})
  }

  async function prepareDataOperation(): Promise<void> {
    if (hasUnsavedDraft.value) requestSave()
    if (saving.value) await new Promise<void>((resolve) => {
      const stop = watch(saving, (value) => { if (!value) { stop(); resolve() } })
    })
    if (hasUnsavedDraft.value) throw new Error(status.value || '设置尚未保存，请先完成编辑')
  }

  async function applyRestoredSettings(restored: Settings): Promise<void> {
    // A background sync must not discard a draft the user is currently editing.
    if (hasUnsavedDraft.value || saving.value) return
    editRevision += 1
    settings.value = restored
    filterListText.value = restored.filterList.join('\n')
    await nextTick()
    apiSettings.value?.reload()
    applyAppearance(restored)
    setStatus('已更新设置与数据', 'success')
  }

  function isTextField(target: EventTarget | null): target is HTMLInputElement | HTMLTextAreaElement {
    return target instanceof HTMLTextAreaElement
      || (target instanceof HTMLInputElement && !['checkbox', 'range', 'color'].includes(target.type))
  }

  function noteDraftEdit(event: Event): void {
    noteLocalDraftChange()
    if (isTextField(event.target)) editedFields.add(event.target)
  }

  function noteLocalDraftChange(): void {
    hasUnsavedDraft.value = true
    signalEditing(true)
    // Every edit invalidates older save responses, including API parameter rows
    // that have not yet been committed into `settings`.
    editRevision += 1
  }

  function handleControlChange(event: Event): void {
    if (!isTextField(event.target)) requestSave()
  }

  function handleControlFocusOut(event: FocusEvent): void {
    if (!isTextField(event.target) || !editedFields.has(event.target)) return
    editedFields.delete(event.target)
    requestSave()
  }

  function handleControlClick(event: MouseEvent): void {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button')
    if (button && !button.hasAttribute('data-no-autosave')) requestSave()
  }

  async function flushSaves(): Promise<void> {
    if (saving.value || !settings.value) return
    saving.value = true
    try {
      while (saveRequested && settings.value) {
        saveRequested = false
        const revision = editRevision
        setStatus('正在自动保存…')
        try {
          apiSettings.value?.commit()
          settings.value.filterList = filterListText.value.split(/\r?\n|,/).map((item) => item.trim()).filter(Boolean)
          const payload: Settings = cloneRaw(settings.value)
          const saved: Settings = await command<Settings>('save_settings', { settings: payload })

          // A newer edit may arrive while Rust is writing the previous snapshot.
          // Never replace that newer local state with an older response.
          if (editRevision === revision) {
            hasUnsavedDraft.value = false
            signalEditing(false)
            settings.value = saved
            filterListText.value = saved.filterList.join('\n')
            await nextTick()
            apiSettings.value?.reload()
            applyAppearance(saved)
            setStatus('已自动保存', 'success')
          } else if (!saveRequested) {
            setStatus('')
          }
        } catch (error) {
          setStatus(errorMessage(error), 'error')
          if (!saveRequested) break
        }
      }
    } finally {
      saving.value = false
      if (saveRequested) void flushSaves()
    }
  }

  onMounted(async () => {
    try {
      settings.value = await command<Settings>('get_settings')
      filterListText.value = settings.value.filterList.join('\n')
      applyAppearance(settings.value)
      const platform = await command<PlatformInfo>('platform_info')
      if (platform.platform === 'linux' && platform.displayProtocol === 'wayland') {
        platformNotice.value = 'Wayland 对全局鼠标监听有限制；如果桌面环境不支持所需协议，请切换到 X11/XWayland。'
      }
      if (!await listenEvent<string>('selection-error', (event) => (serviceError.value = event.payload ?? ''))) return
      if (!await listenEvent<Settings>('settings-restored', (event) => { void applyRestoredSettings(event.payload) })) return
    } catch (error) { setStatus(errorMessage(error), 'error') }
  })

  onBeforeUnmount(() => {
    signalEditing(false)
    window.clearTimeout(statusTimer)
  })

  return { settings, status, statusKind, serviceError, platformNotice, saving, hasUnsavedDraft, filterListText, apiSettings, setStatus, requestSave, prepareDataOperation, noteDraftEdit, noteLocalDraftChange, handleControlChange, handleControlFocusOut, handleControlClick }
}
