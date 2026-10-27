<script setup lang="ts">
import { computed, ref, watch, nextTick, onMounted, onBeforeUnmount } from 'vue';
import { isTauri } from '@tauri-apps/api/core';
import { call } from '../../services/backend';
import { useEditor } from '../editor';
import { samplerBorder } from './samplerAppearance';
const t=useEditor().state.tools;
const active=computed(()=>t.globalPicker||t.temporaryPicker);
const pointer=ref<{x:number;y:number}|null>(null), lens=ref<HTMLCanvasElement>();
const color=ref(''), message=ref(''), picking=ref(false);
let pendingTarget: 'primary'|'secondary'|null=null;
let alive=true,busy=false,suppressClick=false,serial=0,epoch=0,timer:ReturnType<typeof setTimeout>|undefined;
type Sample={color:string;pixels:number[];width:number};
function browserSample(x:number,y:number):Sample {
  const elements=document.elementsFromPoint(x,y);
  for(const element of elements) {
    if(element instanceof HTMLCanvasElement && element.classList.contains('artwork')&&!element.classList.contains('selection-overlay')) {
      const box=element.getBoundingClientRect(),px=Math.floor((x-box.left)/box.width*element.width),py=Math.floor((y-box.top)/box.height*element.height);
      const ctx=element.getContext('2d');if(!ctx)continue;
      const pixels:number[]=[];
      for(let yy=-5;yy<=5;yy++)for(let xx=-5;xx<=5;xx++)pixels.push(...ctx.getImageData(Math.max(0,Math.min(element.width-1,px+xx)),Math.max(0,Math.min(element.height-1,py+yy)),1,1).data);
      return {color:'#'+pixels.slice(240,244).map(n=>n.toString(16).padStart(2,'0')).join(''),pixels,width:11};
    }
    const background=getComputedStyle(element).backgroundColor.match(/[\d.]+/g);
    if(background&&background.length>=3&&(background.length===3||Number(background[3])>0)) {
      const ink=background.slice(0,3).map(Number);return {color:'#'+ink.map(n=>Math.round(n).toString(16).padStart(2,'0')).join(''),pixels:Array.from({length:121},()=>[...ink,255]).flat(),width:11};
    }
  }
  throw new Error('No color at this position');
}
async function sample(x:number,y:number) { return isTauri()?call<Sample>('sample_color',{x,y}):browserSample(x,y); }
function draw(result:Sample) {
  const canvas=lens.value;if(!canvas)return;
  const source=document.createElement('canvas');source.width=source.height=result.width;
  source.getContext('2d')!.putImageData(new ImageData(new Uint8ClampedArray(result.pixels),result.width,result.width),0,0);
  const ctx=canvas.getContext('2d')!;
  const theme=document.querySelector('.workbench-layout')??document.documentElement;
  const styles=getComputedStyle(theme);
  const itemRadius=parseFloat(styles.getPropertyValue('--frame-control-radius'));
  const radius=Math.max(0,Math.min(5,(Number.isFinite(itemRadius)?itemRadius:4)/2));
  ctx.clearRect(0,0,59,77);
  const middle=Math.floor(result.width/2),offset=(middle*result.width+middle)*4;
  const border=samplerBorder(result.pixels.slice(offset,offset+3));
  ctx.clearRect(0,0,59,77);
  ctx.save();ctx.beginPath();ctx.roundRect(0,0,59,77,radius);ctx.clip();
  ctx.fillStyle=`rgb(${border.slice(0,3).join(' ')})`;ctx.fillRect(0,0,59,77);
  ctx.save();ctx.beginPath();ctx.roundRect(2,2,55,55,[Math.max(0,radius-1),Math.max(0,radius-1),0,0]);ctx.clip();
  ctx.imageSmoothingEnabled=false;ctx.drawImage(source,2,2,55,55);ctx.restore();
  ctx.save();ctx.beginPath();ctx.roundRect(2,59,55,16,[0,0,Math.max(0,radius-1),Math.max(0,radius-1)]);ctx.clip();
  ctx.fillStyle=result.color;ctx.fillRect(2,59,55,16);ctx.restore();
  ctx.restore();
  // A small high-contrast crosshair marks the exact sampled screen pixel.
  ctx.lineWidth=3;ctx.strokeStyle='#111';ctx.beginPath();
  ctx.moveTo(19,29.5);ctx.lineTo(25,29.5);ctx.moveTo(34,29.5);ctx.lineTo(40,29.5);
  ctx.moveTo(29.5,19);ctx.lineTo(29.5,25);ctx.moveTo(29.5,34);ctx.lineTo(29.5,40);ctx.stroke();
  ctx.lineWidth=1;ctx.strokeStyle='white';ctx.stroke();
  document.documentElement.style.setProperty('--pixen-sampler-cursor',`url("${canvas.toDataURL()}") 29 29, crosshair`);
}
function clearCursor() {
  document.documentElement.classList.remove('pixen-sampling');
  document.documentElement.style.removeProperty('--pixen-sampler-cursor');
}
async function refresh() {
  if(!active.value||!pointer.value||busy||!alive)return;
  busy=true;const token=serial,point={...pointer.value};
  try { const result=await sample(point.x,point.y);if(alive&&token===serial&&active.value){color.value=result.color;message.value='';await nextTick();draw(result);} }
  catch { if(alive&&token===serial){color.value='';message.value='Color unavailable';} }
  finally {busy=false;if(alive&&active.value)timer=setTimeout(refresh,60);}
}
function move(event:PointerEvent) { pointer.value={x:event.clientX,y:event.clientY};if(active.value&&!busy){clearTimeout(timer);void refresh();} }
function key(event:KeyboardEvent) {
  if(event.key==='Alt'&&!event.repeat&&!event.ctrlKey&&!event.metaKey&&!t.shapeDrawing){event.preventDefault();event.stopImmediatePropagation();t.temporaryPicker=true;}
  if(event.key==='Escape'&&active.value){event.preventDefault();epoch++;t.globalPicker=false;t.temporaryPicker=false;pendingTarget=null;}
}
function up(event:KeyboardEvent){if(event.key==='Alt'){event.preventDefault();event.stopImmediatePropagation();t.temporaryPicker=false;pendingTarget=null;}}
function reset(){epoch++;pendingTarget=null;t.temporaryPicker=false;t.globalPicker=false;pointer.value=null;serial++;clearTimeout(timer);}
function down(event:PointerEvent) {
  if(!active.value||![0,2].includes(event.button))return;
  event.preventDefault();event.stopImmediatePropagation();suppressClick=true;
  pendingTarget=event.button===2?'secondary':'primary';
}
async function release(event:PointerEvent) {
  if(!pendingTarget)return;
  event.preventDefault();event.stopImmediatePropagation();
  const target=pendingTarget,token=epoch;pendingTarget=null;
  picking.value=true;
  try {const result=await sample(event.clientX,event.clientY);if(alive&&token===epoch)t[target]=result.color;}
  catch {message.value='Color unavailable';}
  finally {picking.value=false;t.globalPicker=false;}
}
function click(event:MouseEvent){if(suppressClick||(t.globalPicker||t.temporaryPicker)){event.preventDefault();event.stopImmediatePropagation();suppressClick=false;}}
watch(active,value=>{serial++;clearTimeout(timer);if(value){document.documentElement.classList.add('pixen-sampling');void refresh();}else{color.value='';clearCursor();}});
onMounted(()=>{window.addEventListener('pointermove',move,true);window.addEventListener('pointerdown',down,true);window.addEventListener('pointerup',release,true);window.addEventListener('click',click,true);window.addEventListener('contextmenu',click,true);window.addEventListener('keydown',key,true);window.addEventListener('keyup',up,true);window.addEventListener('blur',reset);document.documentElement.addEventListener('pointerleave',reset);});
onBeforeUnmount(()=>{alive=false;reset();clearCursor();window.removeEventListener('pointermove',move,true);window.removeEventListener('pointerdown',down,true);window.removeEventListener('pointerup',release,true);window.removeEventListener('click',click,true);window.removeEventListener('contextmenu',click,true);window.removeEventListener('keydown',key,true);window.removeEventListener('keyup',up,true);window.removeEventListener('blur',reset);document.documentElement.removeEventListener('pointerleave',reset);});
</script>
<template><Teleport to="body"><div v-if="active&&pointer" class="color-sampler" role="status" :aria-label="message||'Color sampler preview'" :data-color="color"><canvas ref="lens" width="59" height="77" aria-label="Magnified pixels and sampled color" /></div></Teleport></template>
<style>
/* The actual preview is an OS cursor image, excluded from native screen capture. */
.color-sampler { position:fixed; left:-1000px; top:-1000px; width:59px; height:77px; pointer-events:none; }
html.pixen-sampling,html.pixen-sampling * { cursor:var(--pixen-sampler-cursor,crosshair) !important; }
</style>
