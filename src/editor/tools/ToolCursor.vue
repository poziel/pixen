<script setup lang="ts">
import { computed } from 'vue';
import { useEditor } from '../editor';
import { footprint } from './interaction';
import { isShape } from './shapes';
import type { Point } from './raster';
const props=defineProps<{pixel:Point|null;width:number;height:number;scale:number}>();
const t=useEditor().state.tools;
const outlines=computed(()=>{
  if(!props.pixel)return [];
  const shapes=(['left','right'] as const).map(slot=>{
    const tool=t[slot],brush=tool==='pencil'||tool==='eraser'?t.brushes[slot][tool]:isShape(tool)?t.options[slot].shapes[tool]:tool==='paint-select'?t.options[slot].selectionBrush:null;
    return {slot,shape:footprint(tool,props.pixel!,brush?.size??1),kind:brush?.shape??'square'};
  });
  const same=shapes[0].shape&&shapes[1].shape&&shapes[0].shape.size===shapes[1].shape.size&&shapes[0].kind===shapes[1].kind;
  return shapes.flatMap(({slot,shape,kind})=>{
    if(!shape)return [];
    const {x,y,size}=shape,r=x+size,b=y+size,m=x+size/2;
    const radius=size/2;
    const d=kind==='circle'?(same?`M${m} ${y}A${radius} ${radius} 0 0 ${slot==='left'?0:1} ${m} ${b}`:`M${m} ${y}A${radius} ${radius} 0 1 0 ${m} ${b}A${radius} ${radius} 0 1 0 ${m} ${y}`):same?(slot==='left'?`M ${m} ${y} H ${x} V ${b} H ${m}`:`M ${m} ${y} H ${r} V ${b} H ${m}`):`M ${x} ${y} H ${r} V ${b} H ${x} Z`;
    return [{slot,d,...shape}];
  });
});
</script>
<template>
  <svg class="artwork tool-cursor" :width="width" :height="height" :viewBox="`0 0 ${width} ${height}`" aria-hidden="true">
    <g v-for="outline in outlines" :key="outline.slot" :class="`tool-slot-${outline.slot}`" :data-slot="outline.slot" :data-size="outline.size" :data-pixel-x="pixel?.x" :data-pixel-y="pixel?.y">
      <path :d="outline.d" fill="none" stroke="#111" :stroke-width="3.5/scale" />
      <path :d="outline.d" fill="none" stroke="currentColor" :stroke-width="1.5/scale" />
    </g>
  </svg>
</template>
