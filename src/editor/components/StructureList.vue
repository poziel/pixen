<script setup lang="ts">
import { computed, nextTick, ref, onMounted, onBeforeUnmount } from 'vue';
import { FrameAlertDialog, FrameContextMenu, FrameIconButton, FrameReorderList, useFrame } from '@frame/core';
import { Eye, EyeOff, Circle } from '@lucide/vue';
import { useEditor } from '../editor';
import { useStructure, type StructureKind } from '../structure';
import Thumbnail from './Thumbnail.vue';
import PreviewRow from './PreviewRow.vue';
import StructureDragPreview from './StructureDragPreview.vue';
import MatrixDuration from '../timeline/MatrixDuration.vue';
import {durationOperations} from '../timeline/frameTiming';
const props = defineProps<{ kind: StructureKind }>();
const editor = useEditor(), s = editor.state, controller = useStructure(props.kind);
const frame=useFrame();
const visible=computed(()=>!frame.layout.panelVisible&&frame.layout.primarySidebarVisible&&frame.state.activeSidePanel===`pixen.${props.kind==='layer'?'layers':'timeline'}`);
const { display, selection, editing, deletion, disabled } = controller;
const durationEdit=ref<{id:string;generation:number;document:string|null|undefined}>();
function startDuration(id:string){if(disabled.value||!s.snapshot?.document)return;s.playing=false;durationEdit.value={id,generation:s.snapshot.document.generation,document:s.snapshot.active_document};}
async function saveDuration(id:string,value:number){const target=durationEdit.value;durationEdit.value=undefined;if(!target||target.id!==id||disabled.value)return;if(target.document!==s.snapshot?.active_document||target.generation!==s.snapshot?.document?.generation){frame.status.show('Artwork changed. Edit the duration again.',{severity:'error'});return;}await editor.action('edit_structure',{documentId:target.document,generation:target.generation,operation:durationOperations([id],value)});}
const root = ref<HTMLElement>(), menu = ref<{ id: string | null; x: number; y: number } | null>(null);
let contextId: string | null = null;
const ids = computed(() => display.value.map(item => item.id));
const frameId = computed(() => s.snapshot?.document?.frames[s.frame]?.id ?? '');
const menuItems = computed(() => [
  { id: 'add', label: `New ${props.kind}`, disabled: disabled.value },
  ...(props.kind==='frame'?[{id:'add-empty',label:'New empty frame',disabled:disabled.value}]:[]),
  ...(menu.value?.id ? [
    { id: 'rename', label: 'Rename', shortcut: 'F2', disabled: disabled.value },
    { id: 'duplicate', label: 'Duplicate', disabled: disabled.value },
    { id: 'delete', label: 'Delete', separator: true, disabled: disabled.value || display.value.length < 2 },
  ] : []),
]);
function context(event: MouseEvent) {
  event.preventDefault();
  if ((event.target as HTMLElement).closest('input')) return;
  const id = (event.target as HTMLElement).closest<HTMLElement>('[data-reorder-id]')?.dataset.reorderId ?? null;
  if (id) { controller.select(id); if (!selection.value.includes(id)) selection.value = [id]; }
  contextId = id;
  menu.value = { id, x: event.clientX, y: event.clientY };
}
function action(item: { id: string }) {
  const id = contextId; menu.value = null;
  if (disabled.value) return;
  if (item.id === 'add') void controller.add(id ?? undefined);
  if (item.id === 'add-empty') void controller.addEmpty(id??undefined);
  if (!id) return;
  if (item.id === 'rename') editing.value = id;
  if (item.id === 'duplicate') void controller.duplicate(id);
  if (item.id === 'delete') controller.requestDelete(id);
}
function subtitle(id: string) {
  if (props.kind === 'frame') return `${s.snapshot?.document?.frames.find(item => item.id === id)?.duration_ms} ms`;
  const layer = s.snapshot?.document?.layers.find(item => item.id === id);
  return `${Math.round((layer?.opacity ?? 1) * 100)}% opacity${!layer?.visible ? ' · hidden in document' : ''}`;
}
function startRename(id: string) { if (!disabled.value) {controller.select(id); editing.value = id;} }
function selectRow(id: string, event: MouseEvent, select: (event: MouseEvent) => void) {
  if (disabled.value) controller.select(id);
  else select(event);
}
// Keep selection navigation separate from Alt+Arrow reordering and text editing.
function navigate(event: KeyboardEvent) {
  if (!['ArrowUp', 'ArrowDown'].includes(event.key) || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey
    || (event.target as HTMLElement).closest('input,textarea,select,[contenteditable=true]') || editing.value || durationEdit.value) return;
  event.preventDefault();
  event.stopPropagation();
  if (editor.following() || !ids.value.length) return;
  const current = ids.value.indexOf(controller.active.value ?? '');
  const index = current < 0 ? 0 : Math.max(0, Math.min(ids.value.length - 1, current + (event.key === 'ArrowDown' ? 1 : -1)));
  const id = ids.value[index]!;
  controller.select(id);
  selection.value = [id];
  void nextTick(() => {
    const row = Array.from(root.value?.querySelectorAll<HTMLElement>('[data-reorder-id]') ?? []).find(row => row.dataset.reorderId === id);
    row?.querySelector<HTMLElement>('.preview-row-select')?.focus({ preventScroll: true });
    row?.scrollIntoView({ block: 'nearest' });
  });
}
let scrubbing = false;
function beginScrub(event: PointerEvent) {
  if (event.button !== 0 || !visible.value || !root.value) return;
  const element = root.value, target = event.target as HTMLElement, box = element.getBoundingClientRect();
  const bar = target.closest<HTMLElement>('.frame-overlay-scrollbar.y');
  const track = bar?.getBoundingClientRect();
  scrubbing = element.scrollHeight > element.clientHeight + 1 && (
    !!track && Math.abs(track.right - box.right) < 16 && track.top >= box.top - 1 && track.bottom <= box.bottom + 1
    || target === element && event.clientX >= box.left + element.clientWidth
  );
}
function endScrub() { scrubbing = false; }
function scrub() {
  const element = root.value;
  if (!scrubbing || !element || editor.following() || editing.value || durationEdit.value || !ids.value.length) return;
  const range = element.scrollHeight - element.clientHeight;
  if (range <= 1) return;
  const index = Math.max(0, Math.min(ids.value.length - 1, Math.round(element.scrollTop / range * (ids.value.length - 1))));
  const id = ids.value[index]!;
  controller.select(id);
  selection.value = [id];
}
onMounted(() => {
  window.addEventListener('pointerdown', beginScrub, true);
  window.addEventListener('pointerup', endScrub);
  window.addEventListener('pointercancel', endScrub);
  window.addEventListener('blur', endScrub);
});
onBeforeUnmount(() => {
  window.removeEventListener('pointerdown', beginScrub, true);
  window.removeEventListener('pointerup', endScrub);
  window.removeEventListener('pointercancel', endScrub);
  window.removeEventListener('blur', endScrub);
});
function toggle(id: string) { s.hidden = s.hidden.includes(id) ? s.hidden.filter(value => value !== id) : [...s.hidden, id]; }
function solo(id: string) { s.hidden = s.snapshot?.document?.layers.filter(item => item.id !== id).map(item => item.id) ?? []; }
function dialogOpen(value: boolean) {
  if (value) return;
  const target = deletion.value;
  void nextTick(() => { if (deletion.value === target) deletion.value = null; });
}
</script>
<template>
  <div ref="root" class="preview-list structure-list" tabindex="0" @keydown.capture="navigate" @scroll="scrub" @contextmenu="context">
    <p v-if="!s.snapshot?.document" class="empty-note">Open or create artwork to see its {{ kind === 'layer' ? 'layers' : 'timeline' }}.</p>
    <FrameReorderList :model-value="ids" v-model:selected="selection" :disabled="disabled" :label="kind === 'layer' ? 'Layers' : 'Frames'" @activate="controller.select" @reorder="controller.reorder($event.order)">
      <template #drag-preview="{ ids: draggedIds }"><StructureDragPreview :kind="kind" :ids="draggedIds" /></template>
      <template #default="{ id, selected, select }">
        <PreviewRow :name="controller.name(id)" :subtitle="subtitle(id)" :selected="selected || controller.active.value === id" :disabled="editor.following()" :editing="editing === id" :editing-subtitle="durationEdit?.id===id"
          @select="selectRow(id, $event, select)" @rename="startRename(id)" @save="controller.rename(id, $event)" @cancel="editing = null">
          <template v-if="kind==='frame'" #subtitle><MatrixDuration :value="s.snapshot?.document?.frames.find(item=>item.id===id)?.duration_ms??100" :editing="durationEdit?.id===id" :disabled="disabled" :label="`${controller.name(id)} duration (ms)`" @start="startDuration(id)" @save="saveDuration(id,$event)" @cancel="durationEdit=undefined" @invalid="frame.status.show('Enter a duration greater than zero milliseconds.',{severity:'error'})" /></template>
          <template v-if="kind === 'layer'" #actions>
            <FrameIconButton variant="icon" :size="24" :label="`${s.hidden.includes(id) ? 'Show' : 'Hide'} ${controller.name(id)}`" :aria-pressed="!s.hidden.includes(id)" :icon="s.hidden.includes(id) ? EyeOff : Eye" :disabled="editor.following()" @click="toggle(id)" />
            <FrameIconButton variant="icon" :size="24" :label="`Solo ${controller.name(id)}`" tooltip="Show only this layer" :icon="Circle" :disabled="editor.following()" @click="solo(id)" />
          </template>
          <template #preview><Thumbnail :frame-id="kind === 'frame' ? id : frameId" :layer-id="kind === 'layer' ? id : undefined" /></template>
        </PreviewRow>
      </template>
    </FrameReorderList>
    <FrameContextMenu v-if="menu && root" :anchor="root" :point="menu" :items="menuItems" :label="`${kind} actions`" @select="action" @close="menu = null" />
    <FrameAlertDialog :open="!!deletion && visible" :title="`Delete ${kind}?`" :description="`Delete “${deletion?.name ?? ''}” and its artwork? You can undo this change.`" confirm-label="Delete" @confirm="controller.confirmDelete" @update:open="dialogOpen">
      <template #trigger><span /></template>
    </FrameAlertDialog>
  </div>
</template>

<style scoped>
.structure-list { min-height:0;box-sizing:border-box; }
.structure-list :deep(.frame-reorder-list) { min-height:0; }
.structure-list :deep(.matrix-duration),
.structure-list :deep(.matrix-duration .frame-editable-input) { text-align:left; }
</style>
