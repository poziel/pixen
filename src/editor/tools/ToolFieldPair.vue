<script setup lang="ts">
import { ref, watch } from 'vue';
import { FrameScrubField, FrameIconButton } from '@frame/core';
import { Link, Unlink } from '@lucide/vue';
import { updatePair, type FieldPair } from './fieldPair';
const props=withDefaults(defineProps<{
  modelValue:FieldPair; label:string; labels:FieldPairLabels; linkable?:boolean; disabled?:boolean; min?:number; suffix?:string;
}>(),{linkable:false,min:0,suffix:' px'});
type FieldPairLabels=[string,string];
const emit=defineEmits<{'update:modelValue':[value:FieldPair]}>();
const linked=ref(false),ratio=ref<FieldPair|null>(null);
let expected:FieldPair|null=null;
function toggle(){if(props.disabled)return;linked.value=!linked.value;ratio.value=linked.value?[...props.modelValue]:null;}
function update(index:0|1,value:number){
  if(props.disabled)return;
  expected=updatePair(props.modelValue,index,value,linked.value?ratio.value:null,props.min);
  emit('update:modelValue',expected);
}
watch([()=>props.modelValue[0],()=>props.modelValue[1]],([first,second])=>{
  // Keep the captured ratio through rounded edits; recapture after an external resize or new selection.
  if(linked.value&&(!expected||first!==expected[0]||second!==expected[1]))ratio.value=[first,second];
  expected=null;
});
</script>
<template>
  <div class="tool-field-pair" role="group" :aria-label="label" :aria-disabled="disabled">
    <div class="tool-field-pair-title">{{label}}</div>
    <div class="tool-field-pair-controls" :class="{'tool-field-pair-linkable':linkable}">
      <FrameScrubField v-for="index in ([0,1] as const)" :key="index" :model-value="modelValue[index]" :label="labels[index]" :min="min" :step="1" :suffix="suffix" :disabled="disabled" @update:model-value="update(index,$event)" />
      <div v-if="linkable" class="tool-field-pair-link"><FrameIconButton label="Lock aspect ratio" :tooltip="linked?'Unlock aspect ratio':'Lock aspect ratio'" :aria-pressed="linked" :disabled="disabled" :size="24" :icon-size="14" :icon="linked?Link:Unlink" @click="toggle" /></div>
    </div>
  </div>
</template>
<style scoped>
.tool-field-pair { margin-top:12px;min-width:0; }
.tool-field-pair-title { font-size:12px;color:var(--frame-muted);margin-bottom:5px; }
.tool-field-pair-controls { display:grid;grid-template-columns:minmax(0,1fr);gap:4px; }
.tool-field-pair-linkable { grid-template-columns:minmax(0,1fr) 24px; }
.tool-field-pair-controls > .frame-scrub-field { grid-column:1;min-width:0;width:100%;margin:0; }
.tool-field-pair-link { grid-column:2;grid-row:1 / 3;align-self:center; }
.tool-field-pair :deep(.frame-scrub-input) { max-width:none;width:100%; }
</style>
