<script setup lang="ts">
import AppIcon from '@/components/AppIcon.vue'
import Badge from '@/components/ui/Badge.vue'
import Button from '@/components/ui/Button.vue'
import Select from '@/components/ui/Select.vue'
import {
  boolOptions,
  normalizeExtraParameterValue,
  paramTypeOptions,
  type ExtraParameter,
} from './extraParameters'

const parameters = defineModel<ExtraParameter[]>({ required: true })
const emit = defineEmits<{ 'draft-change': [] }>()

function addParameter(): void {
  parameters.value.push({ key: '', type: 'boolean', value: 'false' })
  emit('draft-change')
}
</script>

<template>
  <section class="rounded-2xl border border-border bg-card p-6 shadow-xs">
    <div class="mb-3.5 flex flex-wrap items-center justify-between gap-3">
      <div>
        <div class="flex items-center gap-2">
          <h3 class="m-0 text-[14px] font-semibold tracking-tight text-foreground">额外 API 请求参数</h3>
          <Badge v-if="parameters.length" variant="muted">{{ parameters.length }} 项</Badge>
        </div>
        <span class="mt-1 block text-[11.5px] text-muted-foreground">用于供应商特有参数，例如 <code class="rounded bg-secondary px-1 py-0.5 text-primary">enable_thinking=false</code>；核心字段会由 CitrI 覆盖</span>
      </div>
      <Button size="sm" variant="secondary" data-no-autosave @click="addParameter">
        <AppIcon name="plus" :size="13" />添加参数
      </Button>
    </div>

    <div v-if="!parameters.length" class="rounded-xl border border-dashed border-border py-4 text-center text-xs text-muted-foreground">
      暂无额外参数
    </div>

    <div v-else class="flex flex-col gap-2">
      <div v-for="(parameter, index) in parameters" :key="index" class="grid grid-cols-[1.1fr_108px_1.3fr_34px] items-center gap-2">
        <input
          v-model="parameter.key"
          placeholder="参数名称"
          spellcheck="false"
          class="h-8.5 min-w-0 rounded-lg border border-border bg-background px-2.5 text-xs focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
        />
        <Select
          v-model="parameter.type"
          :options="paramTypeOptions"
          @change="normalizeExtraParameterValue(parameter)"
        />
        <Select
          v-if="parameter.type === 'boolean'"
          v-model="parameter.value"
          :options="boolOptions"
        />
        <input
          v-else
          v-model="parameter.value"
          :placeholder="parameter.type === 'json' ? '{&quot;key&quot;:&quot;value&quot;}' : '参数值'"
          spellcheck="false"
          class="h-8.5 min-w-0 rounded-lg border border-border bg-background px-2.5 text-xs focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
        />
        <Button
          size="icon"
          variant="outline"
          title="删除参数"
          class="h-8.5 w-8.5 hover:border-destructive/50 hover:bg-destructive/10 hover:text-destructive"
          @click="parameters.splice(index, 1)"
        >
          <AppIcon name="trash" :size="14" />
        </Button>
      </div>
    </div>
  </section>
</template>
