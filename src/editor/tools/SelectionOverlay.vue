<script setup lang="ts">
import { computed } from 'vue';
import { useEditor } from '../editor';
import { handlePoint, selectionHandles } from './selection';
import {isSelectionTool} from './toolInfo';
const props = defineProps<{width:number;height:number;scale:number}>();
const s=useEditor().state, v=s.selectionView;
const canTransform=computed(()=>isSelectionTool(s.tools.left)||isSelectionTool(s.tools.right));
const handles=computed(()=>v.bounds?selectionHandles.map(id=>({id,...handlePoint(v.bounds!,id)})):[]);
const guides=computed(()=>{
  const b=v.bounds;if(!b)return [];
  const cx=b.x+b.width/2,cy=b.y+b.height/2;
  return [{x1:0,y1:cy,x2:b.x,y2:cy,label:`${b.x} px`,x:b.x/2,y:cy},
    {x1:b.x+b.width,y1:cy,x2:props.width,y2:cy,label:`${props.width-b.x-b.width} px`,x:(b.x+b.width+props.width)/2,y:cy},
    {x1:cx,y1:0,x2:cx,y2:b.y,label:`${b.y} px`,x:cx,y:b.y/2},
    {x1:cx,y1:b.y+b.height,x2:cx,y2:props.height,label:`${props.height-b.y-b.height} px`,x:cx,y:(b.y+b.height+props.height)/2}];
});
</script>
<template>
  <svg class="artwork selection-boundary" :viewBox="`0 0 ${width} ${height}`" aria-hidden="true" :data-selection-x="v.bounds?.x" :data-selection-y="v.bounds?.y" :data-selection-width="v.bounds?.width" :data-selection-height="v.bounds?.height">
    <g v-if="v.outline" fill="none" :stroke-width="1/scale">
      <path :d="v.outline" stroke="black" />
      <path class="selection-ants" :d="v.outline" stroke="white" :stroke-dasharray="`${4/scale} ${4/scale}`" :style="{'--selection-dash-end':String(-8/scale)}" />
    </g>
    <path v-if="v.polygon" class="polygon-trace-preview" :d="v.polygon" fill="none" stroke="var(--frame-accent)" :stroke-width="1.5/scale" />
    <path v-if="s.shapeGuide" class="shape-tangent-guide" :d="s.shapeGuide" fill="none" stroke="var(--frame-accent)" :stroke-width="1/scale" />
    <circle v-for="(anchor,index) in s.shapeAnchors" :key="`shape-${index}`" class="shape-anchor" :cx="anchor.x+.5" :cy="anchor.y+.5" :r="3/scale" fill="var(--frame-accent)" stroke="white" :stroke-width="1/scale" />
    <g v-if="v.polygonStart" class="polygon-start-indicator" :data-closable="v.polygonClosable">
      <circle :cx="v.polygonStart.x+.5" :cy="v.polygonStart.y+.5" :r="(v.polygonClosable?6:3)/scale" fill="var(--frame-accent)" stroke="white" :stroke-width="1/scale" />
      <text v-if="v.polygonClosable" :x="v.polygonStart.x+.5+10/scale" :y="v.polygonStart.y+.5-9/scale" :font-size="11/scale" fill="white" stroke="#202020" :stroke-width="2/scale" paint-order="stroke">Click to close</text>
    </g>
    <g v-if="v.bounds&&canTransform" class="selection-handles">
      <rect :x="v.bounds.x" :y="v.bounds.y" :width="v.bounds.width" :height="v.bounds.height" fill="none" stroke="#ffffff88" :stroke-width="1/scale" />
      <rect v-for="h in handles" :key="h.id" :data-selection-handle="h.id" :x="h.x-3/scale" :y="h.y-3/scale" :width="6/scale" :height="6/scale" fill="white" stroke="black" :stroke-width="1/scale" />
    </g>
    <g v-if="v.guides&&v.bounds" class="selection-guides">
      <g v-for="(g,i) in guides" :key="i">
        <line v-bind="{x1:g.x1,y1:g.y1,x2:g.x2,y2:g.y2}" stroke="var(--frame-accent)" :stroke-width="1/scale" />
        <text :x="g.x" :y="g.y-4/scale" :font-size="11/scale" text-anchor="middle" fill="white" stroke="#202020" :stroke-width="2/scale" paint-order="stroke">{{g.label}}</text>
      </g>
    </g>
  </svg>
</template>
<style>
.selection-boundary { pointer-events:none;overflow:visible;background:none!important;box-shadow:none!important; }
.selection-ants { animation:selection-march .6s linear infinite; }
@keyframes selection-march { to { stroke-dashoffset:var(--selection-dash-end); } }
@media(prefers-reduced-motion:reduce) { .selection-ants { animation:none; } }
</style>
