<script setup lang="ts">
import {FrameScrubField,FrameCheckbox} from '@frame/core';
import type {Brush} from './raster';
import ToolFieldPair from './ToolFieldPair.vue';
import ToolSelect from './ToolSelect.vue';
const props=defineProps<{modelValue:Brush;eraser?:boolean;stroke?:boolean}>();
const emit=defineEmits<{'update:modelValue':[value:Brush]}>();
function update(patch:Partial<Brush>){emit('update:modelValue',{...props.modelValue,...patch});}
</script>
<template>
 <div class="tool-option-controls">
  <ToolSelect :model-value="modelValue.shape??'square'" :options="[{value:'square',label:'Square brush'},{value:'circle',label:'Round brush'}]" label="Brush shape" @update:model-value="update({shape:$event as 'square'|'circle'})" />
  <FrameScrubField :model-value="modelValue.size" :label="stroke?'Stroke size':'Size'" suffix=" px" :min="1" :max="256" :step="1" @update:model-value="update({size:$event})" />
  <FrameScrubField :model-value="modelValue.density" label="Density" suffix="%" :min="0" :max="100" :step="1" @update:model-value="update({density:$event})" />
  <FrameScrubField :model-value="modelValue.opacity" label="Opacity" suffix="%" :min="0" :max="100" :step="1" @update:model-value="update({opacity:$event})" />
  <template v-if="!eraser&&!stroke">
   <FrameCheckbox :model-value="modelValue.overwrite??false" label="Overwrite color" @update:model-value="update({overwrite:$event})" />
   <FrameCheckbox :model-value="modelValue.filled!==false" label="Solid brush" @update:model-value="update({filled:$event})" />
   <FrameCheckbox :model-value="modelValue.fillInside??false" label="Fill inside" @update:model-value="update({fillInside:$event})" />
   <FrameCheckbox :model-value="modelValue.spacing??false" label="Spacing" @update:model-value="update({spacing:$event})" />
   <ToolFieldPair v-if="modelValue.spacing" label="Gap" :labels="['Gap X','Gap Y']" :model-value="[modelValue.gapX??0,modelValue.gapY??0]" linkable @update:model-value="update({gapX:$event[0],gapY:$event[1]})" />
  </template>
 </div>
</template>
