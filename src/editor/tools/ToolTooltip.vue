<script setup lang="ts">
import {computed,ref,useId} from 'vue';
import {FrameHoverCard} from '@frame/core';
import type {Tool} from './raster';
import {toolInfo,toolTitle} from './toolInfo';
import ToolHelpText from './ToolHelpText.vue';
const props=defineProps<{tool:Tool;label:string}>();
const open=ref(false),id=useId();
const info=computed(()=>toolInfo[props.tool]);
function focus(event:FocusEvent){if((event.target as HTMLElement).matches(':focus-visible'))open.value=true;}
function hide(){open.value=false;}
</script>
<template>
 <FrameHoverCard v-model:open="open" :open-delay="400" :close-delay="60" :id="id" role="tooltip" class="tool-tooltip" :content-props="{side:'top',align:'center'}">
  <template #trigger><slot :describedby="open?id:undefined" :focus="focus" :hide="hide" /></template>
  <strong class="tool-tooltip-title">{{toolTitle(label)}}</strong>
  <div v-if="info.key" class="tool-tooltip-shortcuts">
   <div><code>{{info.key.toUpperCase()}}</code><span>: Left Button</span></div>
   <div><code>Alt+{{info.key.toUpperCase()}}</code><span>: Right Button</span></div>
  </div>
  <div class="tool-tooltip-description">
   <p><ToolHelpText :text="info.description" /></p>
   <p v-for="(paragraph,index) in info.detail?.split('\n')??[]" :key="index"><ToolHelpText :text="paragraph" /></p>
  </div>
 </FrameHoverCard>
</template>
<style>
.tool-tooltip.frame-general-popup { overflow:visible; }
.tool-tooltip { display:grid;gap:12px;padding:12px;width:360px;box-sizing:border-box;font-size:12px;line-height:1.5;color:var(--frame-text); }
.tool-tooltip-title { display:block;font-weight:700;font-size:13px; }
.tool-tooltip-shortcuts { display:grid;gap:5px; }
.tool-tooltip-shortcuts > div { display:flex;align-items:center; }
.tool-tooltip-description { display:grid;gap:7px; }
.tool-tooltip-description p { margin:0; }
.tool-tooltip code { display:inline;font-family:var(--frame-font-mono,monospace);font-size:.95em;font-weight:600;white-space:nowrap;padding:1px 4px;border:1px solid var(--frame-line);border-radius:3px;background:color-mix(in srgb,var(--frame-text) 8%,transparent); }
.drawing-toolbar .tool-grid > .frame-general-anchor { flex:0 0 36px; }
</style>
