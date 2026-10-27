<script setup lang="ts">
import { computed } from 'vue';
import { FrameSelect, type SettingDefinition, type SettingValue } from '@frame/core';
import type { AiSettings } from '../services/aiSettings';
const props = defineProps<{ ai: AiSettings; setting: SettingDefinition; modelValue: SettingValue; inputId: string; disabled?: boolean }>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
const available = computed(() => props.ai.state.tests[props.ai.state.provider] === 'available');
const options = computed(() => {
  const models = props.ai.state.catalogs[props.ai.state.provider] ?? [];
  const saved = String(props.modelValue);
  return [{ value: '', label: 'Provider default' }, ...models.map(model => ({ value: model.id, label: model.label })), ...(saved && !models.some(model => model.id === saved) ? [{ value: saved, label: `${saved} (saved)` }] : [])];
});
</script>
<template><FrameSelect :id="inputId" :model-value="String(modelValue)" :label="setting.label" :options="options" :disabled="disabled || !available" @update:model-value="emit('update:modelValue', $event)" /></template>
