<script setup lang="ts">
import {ref,watch,computed} from 'vue';
import {FrameDialog,FrameNumberField,FrameButton} from '@frame/core';
export interface DurationTarget {ids:string[];duration:number;document:string|null|undefined;generation:number}
const props=defineProps<{target:DurationTarget|undefined;disabled:boolean}>();
const emit=defineEmits<{close:[];apply:[target:DurationTarget,duration:number]}>();
const value=ref(100);watch(()=>props.target,target=>{if(target)value.value=target.duration;});
const valid=computed(()=>Number.isFinite(value.value)&&value.value>0);
function save(){if(props.target&&valid.value&&!props.disabled)emit('apply',props.target,value.value);}
</script>
<template>
 <FrameDialog :open="!!target" :title="target?.ids.length===1?'Frame duration':'Frame durations'" description="How long each frame stays visible before playback advances. This timing is saved with the artwork." @update:open="value=>{if(!value)emit('close');}">
  <template #trigger><span /></template>
  <p v-if="target && target.ids.length>1">Apply the same duration to {{target.ids.length}} selected frames.</p>
  <FrameNumberField v-model="value" label="Duration (ms)" :min="1" :step="1" controls="stacked" :disabled="disabled" @keydown.enter.prevent="save" />
  <p v-if="!valid" role="alert">Enter a duration greater than zero milliseconds.</p>
  <template #actions><FrameButton label="Cancel" @click="emit('close')" /><FrameButton label="Apply" variant="primary" :disabled="disabled||!valid" @click="save" /></template>
 </FrameDialog>
</template>
