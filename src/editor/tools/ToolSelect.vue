<script setup lang="ts">
import {useId} from 'vue';
import {FrameLabel,FrameSelect} from '@frame/core';
defineOptions({inheritAttrs:false});
defineProps<{modelValue:string;label:string;options:readonly {value:string;label:string}[];disabled?:boolean}>();
const emit=defineEmits<{'update:modelValue':[value:string]}>();
const id=useId();
</script>
<template><div class="tool-select"><FrameLabel :for="id">{{label}}</FrameLabel><FrameSelect :id="id" v-bind="$attrs" :model-value="modelValue" :options="options" :label="label" :disabled="disabled" @update:model-value="emit('update:modelValue',$event)" /></div></template>
<style scoped>
.tool-select { display:grid;gap:4px;min-width:0; }
.tool-select :deep(label) { margin:0;color:var(--frame-muted);font-size:12px; }
.tool-select :deep(.frame-select) { width:100%;min-width:0; }
</style>
