<script setup lang="ts">
import { computed, onMounted, ref, toRaw } from 'vue'
import AppIcon from '@/components/AppIcon.vue'
import { command, errorMessage } from '@/lib/bridge'
import type { ApiProfile, ExtraParameter, ExtraParameterType, JsonValue, Settings } from '@/types'

type StatusKind = 'normal' | 'success' | 'error'
const settings = defineModel<Settings>({ required: true })
const emit = defineEmits<{ status: [message: string, kind?: StatusKind] }>()
const testing = ref(false)
const showKey = ref(false)
const extraParameters = ref<ExtraParameter[]>([])
const activeProfile = computed(() => settings.value.apiProfiles.find((profile) => profile.id === settings.value.activeApiProfileId) ?? null)

function parametersFromBody(body: Record<string, JsonValue>): ExtraParameter[] {
  return Object.entries(body).map(([key, value]) => {
    const type: ExtraParameterType = value === null || Array.isArray(value) || typeof value === 'object'
      ? 'json'
      : typeof value === 'boolean'
        ? 'boolean'
        : typeof value === 'number' ? 'number' : 'string'
    return { key, type, value: type === 'json' ? JSON.stringify(value) : String(value) }
  })
}

function readExtraBody(): Record<string, JsonValue> {
  const result: Record<string, JsonValue> = {}
  for (const parameter of extraParameters.value) {
    const key = parameter.key.trim()
    if (!key) continue
    if (Object.prototype.hasOwnProperty.call(result, key)) throw new Error(`额外请求参数名称重复：${key}`)
    if (parameter.type === 'boolean') result[key] = parameter.value === 'true'
    else if (parameter.type === 'number') {
      const value = Number(parameter.value)
      if (!parameter.value.trim() || !Number.isFinite(value)) throw new Error(`参数 ${key} 不是有效数字`)
      result[key] = value
    } else if (parameter.type === 'json') {
      try { result[key] = JSON.parse(parameter.value) }
      catch { throw new Error(`参数 ${key} 不是有效 JSON`) }
    } else result[key] = parameter.value
  }
  return result
}

function commit(): ApiProfile {
  const profile = activeProfile.value
  if (!profile) throw new Error('没有可用的 API 配置')
  profile.name = profile.name.trim() || '未命名 API'
  profile.extraBody = readExtraBody()
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
    emit('status', '已切换，保存后生效')
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
    emit('status', '已添加 API 配置，请填写并保存')
  } catch (error) { emit('status', errorMessage(error), 'error') }
}

function deleteProfile(): void {
  if (settings.value.apiProfiles.length <= 1) return
  const index = settings.value.apiProfiles.findIndex((profile) => profile.id === settings.value.activeApiProfileId)
  settings.value.apiProfiles.splice(index, 1)
  const next = settings.value.apiProfiles[Math.min(Math.max(index, 0), settings.value.apiProfiles.length - 1)]!
  settings.value.activeApiProfileId = next.id
  reload()
  emit('status', '已删除，保存后生效')
}

function changeParameterType(parameter: ExtraParameter): void {
  if (parameter.type === 'boolean' && !['true', 'false'].includes(parameter.value)) parameter.value = 'false'
  if (parameter.type === 'number' && !Number.isFinite(Number(parameter.value))) parameter.value = '0'
  if (parameter.type === 'json' && !parameter.value.trim()) parameter.value = '{}'
}

async function testApi(): Promise<void> {
  testing.value = true
  try {
    const profile = JSON.parse(JSON.stringify(toRaw(commit()))) as ApiProfile
    const result = await command<string>('test_api', { profile })
    emit('status', `连接成功：${result.slice(0, 40)}`, 'success')
  } catch (error) { emit('status', errorMessage(error), 'error') }
  finally { testing.value = false }
}

onMounted(reload)
defineExpose({ commit, reload })
</script>

<template>
  <section class="settings-card api-card">
    <div class="section-title action-heading"><div><h2>API 连接</h2><span>当前配置用于所有 AI 动作</span></div><button class="secondary-button" type="button" @click="addProfile"><AppIcon name="plus" :size="14" />添加 API</button></div>
    <div class="api-profile-layout">
      <aside class="api-profile-sidebar"><span class="caption">API 配置</span><button v-for="profile in settings.apiProfiles" :key="profile.id" type="button" :class="['profile-button', { active: profile.id === settings.activeApiProfileId }]" @click="selectProfile(profile.id)"><AppIcon name="message-circle-question" :size="14" /><span>{{ profile.name }}</span></button></aside>
      <div v-if="activeProfile" class="api-profile-editor">
        <div class="profile-heading"><label class="field"><span>配置名称</span><input v-model="activeProfile.name" maxlength="50" /></label><button class="danger-icon" type="button" title="删除当前 API" :disabled="settings.apiProfiles.length <= 1" @click="deleteProfile"><AppIcon name="trash" :size="15" /></button></div>
        <div class="field-grid">
          <label class="field field-wide"><span>API URL</span><input v-model.trim="activeProfile.apiUrl" type="url" placeholder="https://api.openai.com/v1" spellcheck="false" /><small>可填写基础地址，或完整的 <code>/chat/completions</code> 地址</small></label>
          <label class="field field-wide"><span>API Key</span><div class="input-with-button"><input v-model.trim="activeProfile.apiKey" :type="showKey ? 'text' : 'password'" placeholder="sk-..." autocomplete="off" spellcheck="false" /><button type="button" :title="showKey ? '隐藏 API Key' : '显示 API Key'" @click="showKey = !showKey"><AppIcon :name="showKey ? 'eyeoff' : 'eye'" :size="16" /></button></div><small>仅由 Rust 后端读取并保存在本机，不会发送给工具条和结果窗口</small></label>
          <label class="field"><span>模型</span><input v-model.trim="activeProfile.model" placeholder="gpt-4.1-mini" spellcheck="false" /></label>
          <label class="field"><span>温度 <b>{{ activeProfile.temperature.toFixed(1) }}</b></span><input v-model.number="activeProfile.temperature" type="range" min="0" max="2" step="0.1" /></label>
          <label class="field"><span>最大输出 Token</span><input v-model.number="activeProfile.maxTokens" type="number" min="1" max="128000" /></label>
          <div class="field test-field"><span>连接测试</span><button class="secondary-button" type="button" :disabled="testing" @click="testApi">{{ testing ? '测试中…' : '测试当前 API' }}</button></div>
          <label class="field field-wide"><span>系统 Prompt</span><textarea v-model="activeProfile.systemPrompt" rows="3" /></label>
          <div class="field field-wide"><div class="parameter-heading"><span>额外 API 请求参数</span><button class="secondary-button small-button" type="button" @click="extraParameters.push({ key: '', type: 'boolean', value: 'false' })"><AppIcon name="plus" :size="13" />添加参数</button></div>
            <div v-if="!extraParameters.length" class="parameter-empty">暂无额外参数</div>
            <div v-for="(parameter, index) in extraParameters" :key="index" class="parameter-row">
              <input v-model="parameter.key" placeholder="参数名称" spellcheck="false" />
              <select v-model="parameter.type" @change="changeParameterType(parameter)"><option value="string">字符串</option><option value="boolean">布尔值</option><option value="number">数字</option><option value="json">JSON</option></select>
              <select v-if="parameter.type === 'boolean'" v-model="parameter.value"><option value="true">true</option><option value="false">false</option></select>
              <input v-else v-model="parameter.value" :placeholder="parameter.type === 'json' ? '{&quot;key&quot;:&quot;value&quot;}' : '参数值'" spellcheck="false" />
              <button type="button" title="删除参数" @click="extraParameters.splice(index, 1)"><AppIcon name="trash" :size="14" /></button>
            </div>
            <small>用于供应商特有参数，例如 <code>enable_thinking=false</code>；核心字段会由 CitrI 覆盖。</small>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>
