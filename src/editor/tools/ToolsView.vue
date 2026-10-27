<script setup lang="ts">
import {computed,ref,watch,nextTick} from 'vue';
import {FrameScrubField,FrameCheckbox,FrameButton,FrameTextArea} from '@frame/core';
import { useEditor } from '../editor';
import ToolFieldPair from './ToolFieldPair.vue';
import ToolSelect from './ToolSelect.vue';
import BrushToolOptions from './BrushToolOptions.vue';
import type {FieldPair} from './fieldPair';
import type {SelectionMode} from './selection';
import type {Brush} from './raster';
import {isShape} from './shapes';
import {isSelectionTool} from './toolInfo';
const props=defineProps<{slot:'left'|'right'}>();
const s=useEditor().state,t=s.tools,tool=computed(()=>t[props.slot]),options=computed(()=>t.options[props.slot]);
const settings=ref<HTMLElement>();
watch(tool,async()=>{await nextTick();if(settings.value?.getClientRects().length)settings.value.scrollIntoView({block:'start',inline:'nearest'});});
const currentBrush=computed<Brush>({get:()=>isShape(tool.value)?options.value.shapes[tool.value]:t.brushes[props.slot][tool.value==='eraser'?'eraser':'pencil'],set:value=>{if(isShape(tool.value))options.value.shapes[tool.value]=value;else t.brushes[props.slot][tool.value==='eraser'?'eraser':'pencil']=value;}});
const colorOptions=computed(()=>tool.value==='wand'?options.value.wand:options.value.color);
const selectionModes=[{value:'replace',label:'Replace selection'},{value:'add',label:'Add to selection'},{value:'subtract',label:'Subtract from selection'},{value:'intersect',label:'Intersect selections'}];
function editBounds(keys:['x','y']|['width','height'],value:FieldPair){if(!s.selectionView.bounds||!value.every(Number.isFinite))return;s.selectionEdit.bounds={...s.selectionView.bounds,[keys[0]]:value[0],[keys[1]]:value[1]};s.selectionEdit.serial++;}
</script>
<template>
 <fieldset ref="settings" class="pixel-tools tool-parameters tool-control-group" :disabled="!s.snapshot?.document" :inert="!s.snapshot?.document" :aria-disabled="!s.snapshot?.document">
  <section class="tool-settings" v-bind="{'aria-label':slot+' tool settings'}">
   <BrushToolOptions v-if="tool==='pencil'||tool==='eraser'||isShape(tool)" v-model="currentBrush" :eraser="tool==='eraser'" :stroke="isShape(tool)" />
   <template v-if="tool==='paint-select'">
    <ToolSelect v-model="options.selectionBrush.action" :options="[{value:'select',label:'Select pixels'},{value:'deselect',label:'Deselect pixels'}]" label="Selection brush action" />
    <ToolSelect v-model="options.selectionBrush.shape" :options="[{value:'square',label:'Square brush'},{value:'circle',label:'Round brush'}]" label="Brush shape" />
    <FrameScrubField v-model="options.selectionBrush.size" label="Brush size" suffix=" px" :min="1" :max="256" :step="1" />
   </template>
   <template v-if="isSelectionTool(tool)">
    <ToolSelect :model-value="t.selectionMode" :options="selectionModes" label="Selection mode" @update:model-value="t.selectionMode=$event as SelectionMode" />
    <template v-if="tool==='color'||tool==='wand'">
     <FrameScrubField v-model="colorOptions.tolerance" label="Tolerance" :min="0" :max="255" :step="1" />
     <FrameCheckbox v-model="colorOptions.merged" label="Sample merged visible layers" />
    </template>
    <ToolFieldPair label="Position" :labels="['X','Y']" :model-value="[s.selectionView.bounds?.x??0,s.selectionView.bounds?.y??0]" :disabled="!s.selectionView.bounds" @update:model-value="editBounds(['x','y'],$event)" />
    <ToolFieldPair label="Size" :labels="['Width','Height']" :model-value="[s.selectionView.bounds?.width??0,s.selectionView.bounds?.height??0]" :min="s.selectionView.bounds?1:0" :disabled="!s.selectionView.bounds" linkable @update:model-value="editBounds(['width','height'],$event)" />
   </template>
   <template v-if="tool==='bucket'">
    <ToolSelect v-model="options.bucket.area" :options="[{value:'contiguous',label:'Similar connected area'},{value:'matching',label:'All matching colors'},{value:'selection',label:'Entire selection'}]" label="Fill area" />
    <FrameCheckbox v-model="options.bucket.merged" label="Sample merged visible layers" :disabled="options.bucket.area==='selection'" />
    <FrameScrubField v-model="options.bucket.tolerance" label="Tolerance" :min="0" :max="255" :step="1" :disabled="options.bucket.area==='selection'" />
    <ToolSelect v-model="options.bucket.fill" :options="[{value:'color',label:'Selected color'},{value:'transparent',label:'Transparent'}]" label="Fill with" />
   </template>
   <FrameCheckbox v-if="tool==='move'" v-model="options.move.copy" label="Copy when moving" />
   <template v-if="tool==='text'">
    <FrameTextArea v-model="options.text.content" label="Text" aria-label="Text" :maxlength="512" />
    <FrameScrubField v-model="options.text.size" label="Size (px)" suffix=" px" :min="1" :max="128" :step="1" />
    <ToolSelect v-model="options.text.font" :options="[{value:'monospace',label:'Monospace'},{value:'sans-serif',label:'Sans serif'},{value:'serif',label:'Serif'},{value:'Arial',label:'Arial'},{value:'Verdana',label:'Verdana'},{value:'Georgia',label:'Georgia'},{value:'Courier New',label:'Courier New'}]" label="Font" />
    <FrameCheckbox v-model="options.text.bold" label="Bold" /><FrameCheckbox v-model="options.text.italic" label="Italic" />
    <ToolSelect v-model="options.text.align" :options="[{value:'left',label:'Left'},{value:'center',label:'Center'},{value:'right',label:'Right'},{value:'justify',label:'Justified'}]" label="Text alignment" />
    <FrameCheckbox v-model="options.text.antialias" label="Antialias text" />
   </template>
   <template v-if="tool==='eyedropper'">
    <ToolSelect v-model="options.picker.target" :options="[{value:'button',label:'Clicked mouse button'},{value:'primary',label:'Left color'},{value:'secondary',label:'Right color'}]" label="Pick for" />
    <ToolSelect v-model="options.picker.source" :options="[{value:'layer',label:'Current layer'},{value:'composite',label:'Merged visible layers'}]" label="Pick source" />
    <ToolSelect v-model="options.picker.channels" :options="[{value:'rgba',label:'Color and alpha'},{value:'rgb',label:'Color only'},{value:'alpha',label:'Alpha only'}]" label="Pick mode" />
   </template>
   <template v-if="tool==='zoom'">
    <ToolSelect v-model="options.zoom" :options="[{value:'in',label:'Zoom in'},{value:'out',label:'Zoom out'}]" label="Zoom mode" />
    <FrameButton label="Fit artwork" @click="s.zoomReset++" /><FrameButton label="100%" @click="s.zoomOne++" />
   </template>
   <FrameButton v-if="tool==='pan'" label="Center artboard" @click="s.canvasView.pan={x:0,y:0}" />
   <FrameScrubField v-if="tool==='paper'" v-model="options.paper.opacity" label="Opacity" suffix="%" :min="0" :max="100" :step="1" />
  </section>
 </fieldset>
</template>
<style>
.tool-settings { display:flex;flex-direction:column;gap:8px; }
.tool-settings .frame-select,.tool-settings .frame-textarea { width:100%;min-width:0; }
.tool-settings .frame-scrub-field { min-width:0;margin:0; }
.tool-settings .frame-checkbox { justify-content:flex-start;margin:0; }
.tool-settings .frame-check-control { min-height:0; }
.tool-option-controls { display:flex;flex-direction:column;gap:8px; }
.tool-selection-mode { display:grid;gap:5px;margin-top:4px; }
.tool-selection-mode > span { font-size:12px;color:var(--frame-muted); }
.tool-settings .frame-text-area,.tool-settings textarea { max-width:none; }
</style>
