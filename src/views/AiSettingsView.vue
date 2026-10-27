<script setup lang="ts">
import { computed } from 'vue';
import { FrameSelect, FrameButton } from '@frame/core';
import { useAiSettings, providers } from '../services/aiSettings';
const settings = useAiSettings(), s = settings.state;
const status = computed(() => s.tests[s.provider]);
const models = computed(() => [{ value: '', label: 'Provider default' }, ...(s.catalogs[s.provider] ?? []).map(model => ({ value: model.id, label: model.label }))]);
const selected = computed({ get: () => status.value === 'available' ? s.models[s.provider] ?? '' : '', set: model => { if (status.value === 'available') s.models[s.provider] = model; } });
</script>
<template>
  <section aria-label="AI settings" class="ai-settings">
    <h3>AI art assistant</h3>
    <div class="setting-field">
      <label for="ai-provider" class="setting-label">Provider</label>
      <FrameSelect id="ai-provider" v-model="s.provider" label="AI provider" :options="providers.map(p => ({ value: p.id, label: p.label }))" />
    </div>
    <FrameButton :label="status === 'testing' ? 'Testing provider…' : 'Test provider'" :disabled="status === 'testing'" @click="settings.testProvider()" />
    <p v-if="status === 'testing'" role="status" class="hint">Testing provider and discovering models…</p>
    <p v-else-if="status === 'unavailable'" role="alert" class="provider-error">This provider is not available.</p>
    <p v-else-if="status === 'available'" role="status" class="hint">Provider is available.</p>
    <p v-else class="hint">Test this provider to discover available models.</p>
    <div class="setting-field">
      <label for="ai-model" class="setting-label">Model</label>
      <FrameSelect id="ai-model" v-model="selected" label="AI model" :options="models" :disabled="status !== 'available'" />
    </div>
    <p v-if="status === 'available' && models.length === 1" class="hint">This provider returned no model choices. Its default model will be used.</p>
    <p class="hint">Saved automatically for new conversations. Existing conversations keep their current provider and model.</p>
    <p v-if="s.error" role="alert">{{ s.error }}</p>
  </section>
</template>
<style scoped>
.provider-error { color: var(--frame-danger, #f85149); }
</style>
