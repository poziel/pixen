<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { FrameAlertDialog, FrameDragPreview, FrameButton, FrameContextMenu, FrameIcon, FrameIconButton, FrameScrollArea, useFrame } from '@frame/core';
import { useEditor } from '../editor';
import { useStructure, type Structure, type StructureKind } from '../structure';
import Thumbnail from '../components/Thumbnail.vue';
import StructureDragPreview from '../components/StructureDragPreview.vue';
import TimelineMatrixToolbar, { type MatrixAction } from './TimelineMatrixToolbar.vue';
import { useMatrixSelection } from './useMatrixSelection';
import { cellKey, groupCells } from './matrixSelection';
import {selectionSummary} from './selectionSummary';
import MatrixHeaderName from './MatrixHeaderName.vue';
import { useMatrixReorder } from './useMatrixReorder';
import { useMatrixContext } from './useMatrixContext';
import { useCelClipboard } from './useCelClipboard';
import FrameDurationDialog,{type DurationTarget} from './FrameDurationDialog.vue';
import {durationOperations} from './frameTiming';
import MatrixDuration from './MatrixDuration.vue';

const editor = useEditor(), s = editor.state;
const frame=useFrame();
const layers = useStructure('layer'), frames = useStructure('frame');
const doc = computed(() => s.snapshot?.document);
const rows = computed(() => [...(doc.value?.layers ?? [])].reverse());
const { root, selected, selectedKeys, anchor, frameIds, choose, activate, key, pointerDown, pointerMove, stopDrag } = useMatrixSelection(editor, clear);
const reorder=useMatrixReorder(root,layers,frames);
const renaming=ref<{kind:StructureKind;id:string}>();
function rename(kind:StructureKind,id:string){if(!layers.disabled.value)renaming.value={kind,id};}
function saveName(kind:StructureKind,id:string,value:string){renaming.value=undefined;(kind==='layer'?layers:frames).rename(id,value);}
const clipboard=useCelClipboard(editor,selected,edit,(message,error=false)=>frame.status.show(message,{severity:error?'error':'info',duration:4000}));
function selectFrame(id=frameIds.value[s.frame]){if(editor.following()||!id)return;selected.value=rows.value.map(layer=>({layerId:layer.id,frameId:id}));anchor.value=selected.value[0];}
const durationTarget=ref<DurationTarget>();
const inlineDuration=ref<{id:string;document:string|null|undefined;generation:number}>();
function beginInlineDuration(id:string){if(frames.disabled.value||!doc.value)return;s.playing=false;inlineDuration.value={id,document:s.snapshot?.active_document,generation:doc.value.generation};}
async function saveInlineDuration(id:string,value:number){const target=inlineDuration.value;inlineDuration.value=undefined;if(!target||target.id!==id)return;if(target.document!==s.snapshot?.active_document||target.generation!==doc.value?.generation){frame.status.show('Artwork changed. Edit the duration again.',{severity:'error'});return;}await edit(durationOperations([id],value),true);}
function openDuration(ids:string[]){if(frames.disabled.value||!doc.value)return;s.playing=false;durationTarget.value={ids:[...new Set(ids)],duration:doc.value.frames.find(f=>f.id===ids[0])?.duration_ms??100,document:s.snapshot?.active_document,generation:doc.value.generation};}
async function saveDuration(target:DurationTarget,value:number){durationTarget.value=undefined;if(target.document!==s.snapshot?.active_document||target.generation!==doc.value?.generation){frame.status.show('Artwork changed. Reopen the duration editor.',{severity:'error'});return;}await edit(durationOperations(target.ids,value),true);}
const menu=useMatrixContext(editor,selected,choose,activate,layers,frames,{rename,link:()=>request('link_cells'),unlink,clear,edit,copy:()=>{void clipboard.copy();},cut:()=>{void clipboard.cut();},paste:()=>{void clipboard.paste();},canCopy:()=>clipboard.canCopy.value,canCut:()=>clipboard.canCut.value,canPaste:()=>clipboard.canPaste.value,selectFrame,duration:openDuration});
const layer = computed(() => doc.value?.layers.find(l => l.id === s.layer));
const groups = computed(() => groupCells(selected.value, frameIds.value));
const disabled = computed(() => layers.disabled.value || selected.value.some(cell => doc.value?.layers.find(l => l.id === cell.layerId)?.locked));
const pending = ref<{ type: 'link_cells'; operations: unknown[]; generation: number; document: string | null | undefined }>();
watch(() => s.snapshot?.active_document, () => { pending.value = undefined; durationTarget.value=undefined;inlineDuration.value=undefined;renaming.value=undefined;reorder.stop(); });
watch(() => doc.value?.generation, () => stopDrag());
function request(type: 'link_cells') {
  if (disabled.value || !doc.value) return;
  const activeFrame = doc.value.frames[s.frame]!.id;
  const operations = groups.value.filter(group => group.frameIds.length > 1).map(group => ({
    type, layer_id: group.layerId, frame_ids: group.frameIds,
    source_frame_id: group.frameIds.includes(activeFrame) ? activeFrame : group.frameIds[0],
    ...(type === 'link_cells' ? { link_id: `link-${crypto.randomUUID()}` } : {}),
  }));
  if (operations.length) pending.value = { type, operations, generation: doc.value.generation, document: s.snapshot?.active_document };
}
async function confirm() {
  const operation = pending.value; pending.value = undefined;
  if (!operation || disabled.value || operation.document !== s.snapshot?.active_document || operation.generation !== doc.value?.generation) return;
  await edit(operation.operations);
}
async function edit(operation: unknown, exact=false) {
  if (layers.disabled.value || (!exact && disabled.value) || !doc.value || (Array.isArray(operation)&&!operation.length)) return;
  s.playing = false;
  await editor.action('edit_structure', { documentId: s.snapshot?.active_document, generation: doc.value.generation, operation });
}
function unlink() { void edit(groups.value.map(group => ({ type: 'unlink_cells', layer_id: group.layerId, frame_ids: group.frameIds }))); }
function clear() { if (selected.value.length) void edit(groups.value.map(group => ({ type: 'clear_cells', layer_id: group.layerId, frame_ids: group.frameIds }))); }
function dialogOpen(value: boolean) {
  if (value) return;
  const target = pending.value;
  void nextTick(() => { if (pending.value === target) pending.value = undefined; });
}
function deleteDialog(controller:Structure,value:boolean){if(value)return;const target=controller.deletion.value;void nextTick(()=>{if(controller.deletion.value===target)controller.deletion.value=null;});}
function toggleLayer(id: string) { if (!editor.following()) s.hidden = s.hidden.includes(id) ? s.hidden.filter(l => l !== id) : [...s.hidden, id]; }
const linked = computed(() => selected.value.some(cell => doc.value?.layers.find(l => l.id === cell.layerId)?.cells?.[frameIds.value.indexOf(cell.frameId)]?.link));
const creation = computed<MatrixAction[]>(() => [
  { id:'new-layer', label:'New layer', icon:'add', disabled:layers.disabled.value, run:()=>{void layers.add();} },
  { id:'new-frame', label:'New frame', icon:'add', disabled:frames.disabled.value, run:()=>{void frames.add();} },
  { id:'empty-frame', label:'Empty frame', icon:'new-file', disabled:frames.disabled.value, run:()=>{void frames.addEmpty();} },
]);
const cells = computed<MatrixAction[]>(() => [
  { id:'select-layer', label:'Select layer cels', icon:'list-selection', disabled:editor.following(), run:()=>{if(s.layer){selected.value=frameIds.value.map(frameId=>({layerId:s.layer!,frameId}));anchor.value=selected.value[0];}} },
  { id:'select-frame', label:'Select frame cels', icon:'layout-panel', disabled:editor.following(), run:()=>selectFrame() },
  { id:'link', label:'Link cels', icon:'link', disabled:disabled.value || !groups.value.some(g=>g.frameIds.length>1), run:()=>request('link_cells') },
  { id:'unlink', label:'Unlink selected cels', icon:'link-external', disabled:disabled.value || !linked.value, run:unlink },
  { id:'copy', label:'Copy selected cels', icon:'copy', shortcut:'Ctrl+C', disabled:!clipboard.canCopy.value, run:()=>{void clipboard.copy();} },
  { id:'cut', label:'Cut selected cels', icon:'scissors', shortcut:'Ctrl+X', disabled:!clipboard.canCut.value, run:()=>{void clipboard.cut();} },
  { id:'paste', label:'Paste into selected cels', icon:'clippy', shortcut:'Ctrl+V', disabled:!clipboard.canPaste.value, run:()=>{void clipboard.paste();} },
  { id:'clear', label:'Clear selected cels', icon:'clear-all', disabled:disabled.value || !selected.value.length, run:clear },
]);
const removeStatus=frame.status.register({
  id:'pixen.matrix-selection',kind:'text',label:'Timeline selection',side:'right',order:3,
  available:()=>frame.layout.panelVisible&&!!doc.value,
  get text(){return selectionSummary(selected.value,doc.value?.layers??[]);},
  tooltip:'Drag / Shift: select cels · Ctrl/Cmd: add cels · Delete: clear · Alt + arrows on headers: reorder',
});
const removeSeparator=frame.status.registerSeparator({id:'pixen.matrix-selection.separator',before:'pixen.matrix-selection',allowRemove:false});
onBeforeUnmount(()=>{removeStatus();removeSeparator();});
</script>

<template>
  <div class="timeline-matrix" @keydown.capture="clipboard.key">
    <TimelineMatrixToolbar :creation="creation" :cells="cells" />
    <FrameScrollArea class="matrix-scroll" orientation="both" viewport-label="Layers and frames">
      <div v-if="doc" ref="root" role="grid" aria-label="Animation cels" aria-multiselectable="true" :aria-rowcount="doc.layers.length + 1" :aria-colcount="doc.frames.length + 1" class="matrix-grid" :data-reordering="reorder.dragging.value || undefined" :style="{ '--matrix-columns': doc.frames.length }" @pointermove="pointerMove" @pointerup="stopDrag()" @pointercancel="stopDrag(true)" @lostpointercapture="stopDrag()" @dragstart.prevent @click.capture="reorder.click">
        <div role="row" class="matrix-row matrix-heading">
          <div role="columnheader" class="matrix-corner">Layers / Frames</div>
          <div v-for="(f, index) in doc.frames" :key="f.id" role="columnheader" class="matrix-frame" data-structure-kind="frame" :data-structure-id="f.id" :class="{ current: index === s.frame, 'insert-before':reorder.indicator.value?.kind==='frame'&&reorder.indicator.value.id===f.id&&!reorder.indicator.value.after, 'insert-after':reorder.indicator.value?.kind==='frame'&&reorder.indicator.value.id===f.id&&reorder.indicator.value.after }" @pointerdown="!renaming && reorder.down($event,'frame',f.id)" @keydown="reorder.key($event,'frame',f.id)" @contextmenu="menu.open($event,'frame',s.layer!,f.id)">
            <MatrixHeaderName :label="frames.name(f.id)" :editing="renaming?.kind==='frame'&&renaming.id===f.id" :disabled="frames.disabled.value" :aria-current="index === s.frame ? 'true' : undefined" @activate="choose({layerId:s.layer!,frameId:f.id}, $event)" @rename="rename('frame',f.id)" @save="saveName('frame',f.id,$event)" @cancel="renaming=undefined"><strong>{{ index + 1 }}</strong></MatrixHeaderName>
            <MatrixDuration :value="f.duration_ms" :editing="inlineDuration?.id===f.id" :disabled="frames.disabled.value" :label="`Frame ${index+1} duration (ms)`" @start="beginInlineDuration(f.id)" @save="saveInlineDuration(f.id,$event)" @cancel="inlineDuration=undefined" @invalid="frame.status.show('Enter a duration greater than zero milliseconds.',{severity:'error'})" />
          </div>
        </div>
        <div v-for="(l, row) in rows" :key="l.id" role="row" class="matrix-row" :class="{ 'row-insert-before':reorder.indicator.value?.kind==='layer'&&reorder.indicator.value.id===l.id&&!reorder.indicator.value.after, 'row-insert-after':reorder.indicator.value?.kind==='layer'&&reorder.indicator.value.id===l.id&&reorder.indicator.value.after }">
          <div role="rowheader" class="matrix-layer" data-structure-kind="layer" :data-structure-id="l.id" :class="{ current: l.id === s.layer }" @pointerdown="!renaming && reorder.down($event,'layer',l.id)" @keydown="reorder.key($event,'layer',l.id)" @contextmenu="menu.open($event,'layer',l.id,doc.frames[s.frame]!.id)">
            <FrameIconButton data-reorder-ignore :label="`${s.hidden.includes(l.id) ? 'Show' : 'Hide'} ${l.name}`" :icon="s.hidden.includes(l.id) ? 'eye-closed' : 'eye'" :aria-pressed="!s.hidden.includes(l.id)" :disabled="editor.following()" @click="toggleLayer(l.id)" />
            <MatrixHeaderName :label="l.name" :editing="renaming?.kind==='layer'&&renaming.id===l.id" :disabled="layers.disabled.value" @activate="choose({layerId:l.id,frameId:doc.frames[s.frame]!.id}, $event)" @rename="rename('layer',l.id)" @save="saveName('layer',l.id,$event)" @cancel="renaming=undefined"><span>{{ l.name }}</span><FrameIcon v-if="l.locked" name="lock" /></MatrixHeaderName>
          </div>
          <div v-for="(f, index) in doc.frames" :key="f.id" role="gridcell" :data-cell="`${row}-${index}`" :data-row="row" :data-column="index" :aria-selected="selectedKeys.has(cellKey({layerId:l.id,frameId:f.id}))" class="matrix-cell" :class="{ selected: selectedKeys.has(cellKey({layerId:l.id,frameId:f.id})), active: l.id === s.layer && index === s.frame, playhead: index === s.frame }" @contextmenu="menu.open($event,'cell',l.id,f.id)">
            <FrameButton :aria-label="`${l.name}, frame ${index + 1}${l.cells?.[index]?.link ? ', linked' : ''}${l.cells && !l.cells[index]?.cel ? ', empty' : ''}`" :tabindex="l.id === s.layer && index === s.frame ? 0 : -1" :disabled="editor.following()" @pointerdown="pointerDown($event, {layerId:l.id,frameId:f.id})" @click="(event: MouseEvent) => {if (!event.detail) choose({layerId:l.id,frameId:f.id}, event);}" @keydown="key($event, {layerId:l.id,frameId:f.id})">
              <Thumbnail v-if="!l.cells || l.cells[index]?.cel" :frame-id="f.id" :layer-id="l.id" />
              <span v-else class="matrix-empty" aria-hidden="true">○</span>
              <FrameIcon v-if="l.cells?.[index]?.link" class="matrix-link" name="link" :size="12" />
            </FrameButton>
          </div>
        </div>
      </div>
    </FrameScrollArea>
    <FrameDragPreview v-if="reorder.preview.value" :anchor="root" :x="reorder.preview.value.x" :y="reorder.preview.value.y"><StructureDragPreview :kind="reorder.preview.value.kind" :ids="reorder.preview.value.ids" /></FrameDragPreview>
    <FrameContextMenu v-if="menu.point.value && root" :anchor="root" :point="menu.point.value" :items="menu.items.value" label="Timeline context menu" @select="menu.action" @close="menu.close" />
    <FrameDurationDialog :target="durationTarget" :disabled="frames.disabled.value" @close="durationTarget=undefined" @apply="saveDuration" />
    <FrameAlertDialog v-for="controller in [layers,frames]" :key="controller.kind" :open="frame.layout.panelVisible && !!controller.deletion.value" :title="`Delete ${controller.kind}?`" :description="`Delete “${controller.deletion.value?.name??''}” and its artwork? You can undo this change.`" confirm-label="Delete" @confirm="controller.confirmDelete" @update:open="deleteDialog(controller,$event)"><template #trigger><span /></template></FrameAlertDialog>
    <FrameAlertDialog :open="!!pending" title="Link selected cels?" description="In each selected layer, artwork in the active frame replaces the other selected cels. If that frame isn’t selected, the first selected cel supplies the artwork. You can undo this change." confirm-label="Link cels" @confirm="confirm" @update:open="dialogOpen"><template #trigger><span /></template></FrameAlertDialog>
  </div>
</template>

<style scoped>
.timeline-matrix { height:100%; min-height:0; display:flex; flex-direction:column; color:var(--frame-text); font-size:var(--frame-font-size-body, 12px); }
.matrix-scroll { flex:1; min-height:0; }
.matrix-grid { width:max-content; min-width:100%; user-select:none; touch-action:none; }
.matrix-row { position:relative;display:grid; grid-template-columns:190px repeat(var(--matrix-columns), 64px); width:max-content; min-width:100%; }
.matrix-heading { position:sticky; top:0; z-index:3; }
.matrix-corner,.matrix-layer { position:sticky; left:0; z-index:2; background:var(--frame-panel, var(--frame-bg)); border-right:1px solid var(--frame-line); }
.matrix-corner { display:flex; align-items:center; padding:0 12px; color:var(--frame-muted, var(--frame-text)); }
.matrix-layer { display:flex; align-items:center; padding:0 4px; }
.matrix-layer > :last-child { min-width:0; flex:1; justify-content:flex-start; }
.matrix-layer span { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.matrix-frame { position:relative;background:var(--frame-panel, var(--frame-bg)); }
.matrix-frame > :deep(button) { display:flex; flex-direction:column; width:100%; gap:1px; border-radius:0; }
.matrix-frame:hover,.matrix-frame:focus-within {background:var(--frame-list-hover-background);}
.matrix-frame :deep(button:hover) {background:transparent;}
.matrix-frame small { font-size:10px; opacity:.65; }
.matrix-frame strong {max-width:100%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.matrix-layer,.matrix-frame {cursor:grab;}
.matrix-layer :deep(button:not([data-reorder-ignore])),.matrix-frame :deep(button){cursor:grab;}
.matrix-grid[data-reordering] .matrix-layer,.matrix-grid[data-reordering] .matrix-frame {cursor:grabbing;}
.matrix-grid[data-reordering] :deep(button){cursor:grabbing;}
.row-insert-before::after,.row-insert-after::after {content:'';position:absolute;left:0;right:0;height:2px;background:var(--frame-accent);z-index:5;pointer-events:none;}
.row-insert-before::after{top:0;}.row-insert-after::after{bottom:0;}
.matrix-frame.insert-before::after,.matrix-frame.insert-after::after{content:'';position:absolute;top:0;bottom:0;width:3px;background:var(--frame-accent);z-index:5;pointer-events:none;}
.matrix-frame.insert-before::after{left:0;}.matrix-frame.insert-after::after{right:0;}
.matrix-cell { height:44px; border-right:1px solid var(--frame-line); border-bottom:1px solid var(--frame-line); padding:3px; }
.matrix-cell > :deep(button) { width:100%; height:100%; padding:3px; position:relative; border-radius:3px; }
.matrix-cell :deep(.thumbnail) { width:32px; height:32px; }
.matrix-cell :deep(img) { width:100%; height:100%; object-fit:contain; image-rendering:pixelated; }
.matrix-cell.playhead,.current { background:color-mix(in srgb, var(--frame-accent, #38a8ee) 9%, var(--frame-panel, var(--frame-bg))); }
.matrix-cell.selected { background:color-mix(in srgb, var(--frame-accent, #38a8ee) 22%, transparent); }
.matrix-cell.active > :deep(button) { outline:2px solid var(--frame-accent, #38a8ee); outline-offset:-1px; }
.matrix-cell :deep(button:focus-visible) { outline:2px solid var(--frame-focus-border, var(--frame-accent, #38a8ee)); }
.matrix-link { position:absolute; bottom:1px; right:1px; background:var(--frame-panel, var(--frame-bg)); }
.matrix-empty { opacity:.45; font-size:18px; }
</style>
