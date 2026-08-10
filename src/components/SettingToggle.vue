<script setup lang="ts">
defineProps<{ title: string; description: string }>()
const model = defineModel<boolean>({ required: true })
</script>

<template>
  <div class="setting-row">
    <div><strong>{{ title }}</strong><p>{{ description }}</p></div>
    <label class="switch"><input v-model="model" type="checkbox" /><span /></label>
  </div>
</template>
