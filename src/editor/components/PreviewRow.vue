<script setup lang="ts">
defineProps<{ name: string; subtitle: string; selected: boolean; disabled?: boolean }>();
defineEmits<{ select: [] }>();
</script>
<template>
  <div class="preview-row" :class="{ selected }">
    <div v-if="$slots.actions" class="preview-row-actions"><slot name="actions" /></div>
    <button type="button" class="preview-row-select" :aria-pressed="selected" :disabled="disabled" @click="$emit('select')">
      <slot name="preview" />
      <span class="preview-row-text"><strong>{{ name }}</strong><small>{{ subtitle }}</small></span>
    </button>
  </div>
</template>
