<script setup lang="ts">
import { computed, ref } from 'vue';
import { FrameTabs, type FrameColor } from '@frame/core';
import ColorChannelRow from './ColorChannelRow.vue';
import { bytesHsb, clamp, hsbBytes, linearGradient, linearToSrgb, srgbToLinear } from './colorMath';
const props = defineProps<{ color: Extract<FrameColor,{space:'hsb'}>; intensity: number }>();
const emit = defineEmits<{ 'update:color': [value: Extract<FrameColor,{space:'hsb'}>]; 'update:intensity': [value: number] }>();
const mode = ref('rgb');
const models = [{value:'rgb',label:'RGB'},{value:'hsv',label:'HSV'},{value:'linear',label:'Linear'}];
const bytes = computed(()=>hsbBytes(props.color));
const rgb = computed(()=>({space:'rgb' as const,r:bytes.value[0],g:bytes.value[1],b:bytes.value[2],alpha:props.color.alpha}));
const linear = computed(()=>({space:'rgb' as const,r:srgbToLinear(bytes.value[0]/255)*255,g:srgbToLinear(bytes.value[1]/255)*255,b:srgbToLinear(bytes.value[2]/255)*255,alpha:props.color.alpha}));
function publish(color: typeof props.color) { emit('update:color',color); }
function channel(index: number,value: number,isLinear=false) {
  const next=[...bytes.value];next[index]=Math.round((isLinear?linearToSrgb(clamp(value))*255:clamp(value,0,255)));
  publish(bytesHsb(next,props.color.h));
}
function hsv(channel:'h'|'s'|'b',value:number) { publish({...props.color,[channel]:value}); }
function exposure(value:number) { emit('update:intensity',value); }
function alpha(value:number,isLinear=false) {
  publish({...props.color,alpha:clamp(isLinear?value:value/255)});
}
</script>
<template>
  <div class="advanced-color-options">
    <FrameTabs v-model="mode" :items="models" label="Color model"><template #content="{item}">
      <template v-if="item.value==='hsv'">
        <ColorChannelRow label="H" name="Hue" :model-value="color.h" :min="0" :max="359" :step="1" :color="color" channel="hue" color-space="hsb" @update:model-value="hsv('h',$event)" />
        <ColorChannelRow label="S" name="Saturation" :model-value="color.s" :min="0" :max="100" :step="1" :color="color" channel="saturation" color-space="hsb" @update:model-value="hsv('s',$event)" />
        <ColorChannelRow label="V" name="Value" :model-value="color.b" :min="0" :max="100" :step="1" :color="color" channel="brightness" color-space="hsb" @update:model-value="hsv('b',$event)" />
      </template>
      <template v-else>
        <ColorChannelRow v-for="(row,index) in ([['R','Red','red'],['G','Green','green'],['B','Blue','blue']] as const)" :key="row[0]" :label="row[0]" :name="row[1]" :model-value="item.value==='linear'?srgbToLinear(bytes[index]/255):bytes[index]" :min="0" :max="item.value==='linear'?1:255" :step="item.value==='linear'?.001:1" :color="item.value==='linear'?linear:rgb" :channel="row[2]" color-space="rgb" :factor="item.value==='linear'?1/255:1" :gradient="item.value==='linear'?linearGradient(bytes,index):undefined" @update:model-value="channel(index,$event,item.value==='linear')" />
      </template>
      <ColorChannelRow label="I" name="Intensity" :model-value="intensity" :min="-8" :max="8" :step=".01" @update:model-value="exposure" />
      <ColorChannelRow label="A" name="Alpha" :model-value="item.value==='linear'?color.alpha:Math.round(color.alpha*255)" :min="0" :max="item.value==='linear'?1:255" :step="item.value==='linear'?.001:1" :factor="item.value==='linear'?1:255" :color="rgb" channel="alpha" color-space="rgb" @update:model-value="alpha($event,item.value==='linear')" />
    </template></FrameTabs>
  </div>
</template>
