<script setup lang="ts">
import { computed } from 'vue';
import { FrameFieldGroup, FrameSelect, FrameButton, type SettingDefinition, type SettingValue } from '@frame/core';
import type { AiSettings } from '../services/aiSettings';
const props = defineProps<{ ai: AiSettings; setting: SettingDefinition; modelValue: SettingValue; inputId: string; disabled?: boolean }>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
const status = computed(() => props.ai.state.tests[props.ai.state.provider]);
</script>
<template><div class="provider-field">
  <FrameFieldGroup :disabled="disabled">
    <FrameSelect :id="inputId" :model-value="String(modelValue)" :label="setting.label" :options="setting.options ?? []" :disabled="disabled" @update:model-value="emit('update:modelValue', $event)" />
    <template #suffix><FrameButton :label="status === 'testing' ? 'Testing…' : 'Test provider'" :disabled="disabled || status === 'testing'" @click="ai.testProvider()" /></template>
  </FrameFieldGroup>
  <p v-if="status === 'testing'" role="status" class="hint">Testing provider and discovering models…</p>
  <p v-else-if="status === 'unavailable'" role="alert" class="provider-error">This provider is not available.</p>
  <p v-else-if="status === 'available'" role="status" class="hint">Provider is available.</p>
  <p v-if="ai.state.error" role="alert">{{ ai.state.error }}</p>
</div></template>
<style scoped>.provider-field{width:100%;min-width:0}.provider-error{color:var(--frame-danger,#f85149)}</style>
