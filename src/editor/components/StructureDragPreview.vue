<script setup lang="ts">
import {computed} from 'vue';
import {useEditor} from '../editor';
import {useStructure,type StructureKind} from '../structure';
import Thumbnail from './Thumbnail.vue';
const props=defineProps<{kind:StructureKind;ids:readonly string[]}>();
const editor=useEditor(),controller=useStructure(props.kind);
const frameId=computed(()=>editor.state.snapshot?.document?.frames[editor.state.frame]?.id??'');
</script>
<template>
 <div class="structure-drag-preview">
  <strong v-if="ids.length>1">{{ids.length}} {{kind==='layer'?'layers':'frames'}}</strong>
  <div v-for="id in ids.slice(0,3)" :key="id" class="structure-drag-item">
   <Thumbnail :frame-id="kind==='frame'?id:frameId" :layer-id="kind==='layer'?id:undefined" />
   <span>{{controller.name(id)}}</span>
  </div>
  <small v-if="ids.length>3">and {{ids.length-3}} more</small>
 </div>
</template>
<style scoped>
.structure-drag-preview{display:grid;gap:5px;width:200px;}
.structure-drag-item{display:flex;align-items:center;gap:8px;min-width:0;}
.structure-drag-item span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.structure-drag-item :deep(.thumbnail){width:32px;height:32px;flex-shrink:0;}
.structure-drag-item :deep(img){width:32px;height:32px;object-fit:contain;}
</style>
