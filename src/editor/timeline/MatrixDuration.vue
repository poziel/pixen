<script setup lang="ts">
import {FrameEditable,FrameTooltip} from '@frame/core';
const props=defineProps<{value:number;editing:boolean;disabled:boolean;label:string}>();
const emit=defineEmits<{start:[];save:[value:number];cancel:[];invalid:[]}>();
function save(raw:string){const value=Number(raw.trim());if(!raw.trim()||!Number.isFinite(value)||value<=0){emit('invalid');emit('cancel');return;}emit('save',value);}
</script>
<template>
 <div class="matrix-duration" :data-reorder-ignore="editing||undefined">
  <FrameEditable v-if="editing" :model-value="String(value)" :disabled="disabled" :label="label" start-with-edit-mode select-on-focus activation-mode="none" submit-mode="both" @submit="save" @cancel="emit('cancel')" />
  <FrameTooltip v-else text="Double-click to edit frame duration"><small :aria-label="label" @dblclick.stop.prevent="!disabled&&emit('start')">{{value}} ms</small></FrameTooltip>
 </div>
</template>
<style scoped>
.matrix-duration{min-width:0;width:100%;text-align:center;line-height:16px;}
.matrix-duration small{font-size:10px;opacity:.65;cursor:text;}
.matrix-duration :deep(.frame-editable-input){width:100%;min-width:0;height:20px;padding:0 2px;font-size:11px;text-align:center;}
</style>
