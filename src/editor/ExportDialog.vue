<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watchEffect } from 'vue';
import { FrameButton, FrameField, FrameTextField, useFrame } from '@frame/core';
import { call } from '../services/backend';
import { useEditor } from './editor';
import ExportEntryView from './ExportEntry.vue';
import { createExportEntry, exportNames, needsBackground, validExport, validExportName } from './exportOptions';
const editor = useEditor(), s = editor.state, frame = useFrame();
const documentId = s.snapshot?.active_document;
const doc = s.snapshot?.document;
const count = doc?.source_frame_count ?? doc?.frames.length ?? 1;
const width = doc?.source_width ?? 1, height = doc?.source_height ?? 1;
let nextId = 1;
const entries = ref([createExportEntry(nextId++, count, s.frame)]);
const name = ref((doc?.path?.split(/[\\/]/).pop() ?? 'artwork').replace(/\.(pixen\.json|pixloom\.json|[^.]+)$/i, ''));
const directory = ref(''), choosing = ref(false);
const filenames = computed(() => exportNames(name.value, entries.value));
const valid = computed(() => !choosing.value && validExportName(name.value) && !!directory.value.trim()
  && entries.value.length > 0 && entries.value.length <= 32
  && entries.value.every(entry => validExport(entry, count, width, height)));
watchEffect(() => { s.exportValid = valid.value; });
onBeforeUnmount(() => { s.exportValid = false; s.dialog = ''; });
function add() { entries.value.push(createExportEntry(nextId++, count, s.frame)); }
async function browse() {
  choosing.value = true; s.error = '';
  try { const selected = await call<string | null>('choose_export_folder'); if (selected) directory.value = selected; }
  catch (error) { s.error = String(error); }
  finally { choosing.value = false; }
}
async function submit() {
  if (!valid.value || s.working || !documentId) return;
  const result = await editor.action('export_document', { documentId, request: {
    directory: directory.value.trim(), name: name.value,
    exports: entries.value.map(entry => ({ format: entry.format, mode: entry.mode, frame: entry.frame - 1,
      scale: entry.scale, gridAxis: entry.gridAxis, gridCount: entry.gridCount,
      background: entry.mode !== 'project' && (entry.opaque || needsBackground(entry.format)) ? entry.background : null,
      loopAnimation: entry.loopAnimation })),
  }});
  if (result === true) {
    frame.status.show(`Exported ${entries.value.length} ${entries.value.length === 1 ? 'file' : 'files'}`, { duration: 2500 });
    frame.closeModal('pixen.dialog');
  }
}
</script>
<template>
<form id="pixen-export" class="dialog-content export-dialog" @submit.prevent="submit">
  <fieldset :disabled="s.working || choosing" class="export-destination">
    <FrameField label="File name"><FrameTextField v-model="name" aria-label="File name" maxlength="120" required autofocus /></FrameField>
    <div class="export-folder"><FrameField label="Folder"><FrameTextField v-model="directory" aria-label="Folder" required placeholder="Choose an export folder" /></FrameField><FrameButton :label="choosing ? 'Choosing…' : 'Browse…'" :disabled="s.working || choosing" @click="browse" /></div>
    <p v-if="!validExportName(name)" role="alert">Enter a file name without path separators or reserved filename characters.</p>
    <p class="hint">Files share this name. Repeated extensions include the export kind to keep each file distinct.</p>
  </fieldset>
  <ExportEntryView v-for="(entry, index) in entries" :key="entry.id" :entry="entry" :index="index" :count="count" :width="width" :height="height" :filename="filenames[index]!" :disabled="s.working || choosing" @remove="entries.splice(index, 1)" />
  <p v-if="!entries.length" class="hint">Add an export to choose what to create.</p>
  <div><FrameButton label="Add export" :disabled="s.working || choosing || entries.length >= 32" @click="add" /></div>
  <p v-if="s.error" role="alert">{{ s.error }}</p>
</form>
</template>
<style scoped>
.export-dialog, .export-destination { display: flex; flex-direction: column; gap: 16px; }
.export-destination { padding: 0; border: 0; margin: 0; min-width: 0; }
.export-folder { display: flex; align-items: end; gap: 8px; }
.export-folder :deep(.frame-field) { flex: 1; min-width: 0; }
.export-dialog p { margin: 0; }
.export-destination :deep(.frame-text-input) { width: 100%; max-width: none; box-sizing: border-box; }
</style>
