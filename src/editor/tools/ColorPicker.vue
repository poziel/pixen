<script setup lang="ts">
import { computed, reactive, ref, watch, onMounted, onBeforeUnmount } from 'vue';
import { FrameHsvRectangle, FrameHsvWheel, FrameVhsCircle, FrameColorSwatch, FrameTextField, FrameDropdownMenu, FrameIconButton, FrameTooltip, type FrameColor } from '@frame/core';
import { useEditor } from '../editor';
import { hexToHsv, rgba } from './raster';
import { rememberColor } from './recent';
import { exposeColor, hsbBytes } from './colorMath';
import AdvancedColorOptions from './AdvancedColorOptions.vue';
const s = useEditor().state;
const t = s.tools;
const color = computed(() => t[t.colorTarget]);
const pickerModes = computed(() => [{id:'rectangle',label:'HSV Rectangle'},{id:'wheel',label:'HSV Wheel'},{id:'circle',label:'VHS Circle'}].map(item=>({...item,checked:t.picker===item.id})));
function selectPicker(item: {id:string}) { t.picker=item.id as typeof t.picker; }
const recentRow = ref<HTMLElement>();
const recentSlots = ref(12);
let recentObserver: ResizeObserver | undefined;
onMounted(() => {
  recentObserver = new ResizeObserver(([entry]) => { recentSlots.value = Math.max(1,Math.min(32,Math.floor((entry.contentRect.width+3)/23))); });
  if (recentRow.value) recentObserver.observe(recentRow.value);
});
onBeforeUnmount(() => recentObserver?.disconnect());
type BaseColor = Extract<FrameColor,{space:'hsb'}>;
function baseColor(value: string): BaseColor {
  const [h,s,b] = hexToHsv(value);
  return {space:'hsb',h,s:s*100,b:b*100,alpha:rgba(value)[3]/255};
}
const bases = reactive({primary:baseColor(t.primary),secondary:baseColor(t.secondary)});
const intensities = reactive({primary:0,secondary:0});
const authored: Partial<Record<'primary'|'secondary',string>> = {};
for (const target of ['primary','secondary'] as const) watch(()=>t[target],value=>{
  if (value===authored[target]) return;
  bases[target]=baseColor(value);intensities[target]=0;
}, {flush:'sync'});
const areaColor = computed(()=>bases[t.colorTarget]);
const intensity = computed(()=>intensities[t.colorTarget]);
function output(target: 'primary'|'secondary') {
  const bytes = exposeColor(hsbBytes(bases[target]),intensities[target]);
  authored[target]='#'+bytes.slice(0,bytes[3]===255?3:4).map(value=>value.toString(16).padStart(2,'0')).join('');
  t[target]=authored[target]!;
}
function areaUpdate(value: FrameColor) {
  if (value.space!=='hsb') return;
  bases[t.colorTarget]={...value};output(t.colorTarget);
}
function intensityUpdate(value: number) { intensities[t.colorTarget]=value;output(t.colorTarget); }
function selectColor(value: string) {
  bases[t.colorTarget]=baseColor(value);intensities[t.colorTarget]=0;output(t.colorTarget);
}
function swapColors() {
  [bases.primary,bases.secondary]=[bases.secondary,bases.primary];
  [intensities.primary,intensities.secondary]=[intensities.secondary,intensities.primary];
  output('primary');output('secondary');
}
function hex(event: Event) {
  const value = (event.target as HTMLInputElement).value.replace(/^#/,'');
  if (/^[\da-f]{6}([\da-f]{2})?$/i.test(value)) {
    selectColor('#'+value);
    rememberColor(t.recentColors, '#' + value);
  } else (event.target as HTMLInputElement).value = color.value.slice(1);
}
</script>
<template>
  <fieldset class="color-picker pixel-tools tool-control-group" :disabled="!s.snapshot?.document" :inert="!s.snapshot?.document" :aria-disabled="!s.snapshot?.document" aria-label="Color picker">
    <component :is="t.picker==='wheel'?FrameHsvWheel:t.picker==='circle'?FrameVhsCircle:FrameHsvRectangle" :model-value="areaColor" size="100%" aria-label="Color surface" @update:color="areaUpdate" />
    <div class="swatches">
      <FrameTooltip text="Left tool color" v-slot="{describedby}"><button :style="{background:t.primary}" :aria-label="`primary color ${t.primary}`" :aria-describedby="describedby" :aria-pressed="t.colorTarget==='primary'" @click="t.colorTarget='primary'"></button></FrameTooltip>
      <FrameIconButton class="color-action" label="Swap colors (X)" shortcut="X" icon="arrow-swap" variant="icon" :hover-highlight="false" @click="swapColors" />
      <FrameTooltip text="Right tool color" v-slot="{describedby}"><button :style="{background:t.secondary}" :aria-label="`secondary color ${t.secondary}`" :aria-describedby="describedby" :aria-pressed="t.colorTarget==='secondary'" @click="t.colorTarget='secondary'"></button></FrameTooltip>
      <FrameIconButton class="color-action" label="Pick color from artwork" shortcut="Alt" icon="color-mode" :aria-pressed="t.globalPicker" @click="t.globalPicker=!t.globalPicker" />
      <FrameTextField :model-value="color.replace(/^#/, '')" aria-label="Hex color, optional alpha" maxlength="9" @change="hex" />
      <FrameDropdownMenu :items="pickerModes" label="Color picker mode" @select="selectPicker"><template #trigger><FrameIconButton class="color-action" label="Color picker type" icon="color-mode" /></template></FrameDropdownMenu>
    </div>

    <div ref="recentRow" class="recent-colors" role="group" aria-label="Recent colors">
      <template v-for="index in recentSlots" :key="index">
        <FrameTooltip v-if="t.recentColors[index-1]" :text="t.recentColors[index-1]" v-slot="{describedby}"><button :aria-label="`Use recent color ${t.recentColors[index-1]}`" :aria-describedby="describedby" @click="selectColor(t.recentColors[index-1])"><FrameColorSwatch :color="t.recentColors[index-1]" /></button></FrameTooltip>
        <FrameColorSwatch v-else class="empty-recent-color" color="transparent" label="Empty recent color" />
      </template>
    </div>
    <div class="color-options">
    <AdvancedColorOptions :color="areaColor" :intensity="intensity" @update:intensity="intensityUpdate" @update:color="areaUpdate" />
    </div>
  </fieldset>
</template>


<style scoped>
.color-picker.pixel-tools { width:100%;max-width:340px;box-sizing:border-box; }
</style>
