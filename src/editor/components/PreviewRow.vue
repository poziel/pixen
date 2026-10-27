<script setup lang="ts">
import { FrameEditable } from '@frame/core';
const props = defineProps<{ name: string; subtitle: string; selected: boolean; disabled?: boolean; editing?: boolean; editingSubtitle?:boolean }>();
const emit = defineEmits<{ select: [event: MouseEvent]; rename: []; save: [value: string]; cancel: [] }>();
</script>
<template>
  <div class="preview-row" :class="{ selected }">
    <div v-if="$slots.actions" class="preview-row-actions"><slot name="actions" /></div>
    <div v-if="editing" class="preview-row-select">
      <slot name="preview" />
      <FrameEditable :model-value="name" :label="`Rename ${name}`" start-with-edit-mode select-on-focus activation-mode="none" submit-mode="both" @submit="emit('save', $event)" @cancel="emit('cancel')" />
    </div>
    <div v-else-if="editingSubtitle" class="preview-row-select"><slot name="preview" /><span class="preview-row-text"><strong>{{name}}</strong><slot name="subtitle"><small>{{subtitle}}</small></slot></span></div>
    <button v-else type="button" class="preview-row-select" :aria-pressed="selected" :disabled="disabled" @click="emit('select', $event)" @keydown.f2.prevent="emit('rename')">
      <slot name="preview" />
      <span class="preview-row-text"><strong @dblclick.stop="emit('rename')">{{ name }}</strong><slot name="subtitle"><small>{{ subtitle }}</small></slot></span>
    </button>
  </div>
</template>
<style scoped>
.preview-row-select > .frame-editable { flex: 1; min-width: 0; }
.preview-row-select :deep(.frame-editable-input) { width: 100%; min-width: 0; }
</style>
