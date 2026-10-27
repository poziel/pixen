<script setup lang="ts">
import { FrameButton, FrameEditable } from '@frame/core';
const props=defineProps<{label:string;editing:boolean;disabled?:boolean}>();
const emit=defineEmits<{activate:[event:MouseEvent];rename:[];save:[value:string];cancel:[]}>();
</script>
<template>
  <FrameEditable v-if="editing" :model-value="label" :disabled="disabled" class="matrix-header-input" :label="`Rename ${label}`" start-with-edit-mode select-on-focus activation-mode="none" submit-mode="both" @submit="emit('save',$event)" @cancel="emit('cancel')" />
  <FrameButton v-else :aria-label="label" :tooltip="label" :disabled="disabled" @click="emit('activate',$event)" @dblclick.stop="!disabled && emit('rename')" @keydown.f2.prevent.stop="!disabled && emit('rename')"><slot>{{label}}</slot></FrameButton>
</template>
<style scoped>.matrix-header-input{width:100%;min-width:0;}.matrix-header-input :deep(.frame-editable-input){width:100%;min-width:0;}</style>
