<script setup lang="ts">
import { computed } from 'vue';
import { FrameCheckbox, FrameColorPicker, FrameField, FrameIconButton, FrameNumberField, FrameSelect } from '@frame/core';
import { Trash2 } from '@lucide/vue';
import { changeExportKind, exportDimensions, exportKinds, formatOptions, needsBackground, validExport, type ExportEntry, type ExportKind } from './exportOptions';
const props = defineProps<{ entry: ExportEntry; index: number; count: number; width: number; height: number; filename: string; disabled: boolean }>();
defineEmits<{ remove: [] }>();
const size = computed(() => exportDimensions(props.entry, props.count, props.width, props.height));
const valid = computed(() => validExport(props.entry, props.count, props.width, props.height));
const kinds = computed(() => exportKinds.map(kind => ({ ...kind, disabled: kind.value === 'video' && props.count < 2 })));
const backgroundVisible = computed(() => props.entry.opaque || needsBackground(props.entry.format));
</script>
<template>
<section class="export-entry" :aria-label="`Export ${index + 1}`">
  <div class="export-entry-header">
    <strong>Export {{ index + 1 }}</strong>
    <FrameIconButton :label="`Remove export ${index + 1}`" :icon="Trash2" variant="icon" :disabled="disabled" @click="$emit('remove')" />
  </div>
  <div class="export-field-pair">
    <FrameField label="Export"><FrameSelect :model-value="entry.mode" :label="`Export kind ${index + 1}`" :options="kinds" :disabled="disabled" @update:model-value="changeExportKind(entry, $event as ExportKind)" /></FrameField>
    <FrameField label="Format"><FrameSelect v-model="entry.format" :label="`Format ${index + 1}`" :options="formatOptions(entry.mode)" :disabled="disabled" /></FrameField>
  </div>
  <fieldset v-if="entry.mode !== 'project'" :disabled="disabled" class="export-entry-fields">
    <div class="export-field-pair">
      <FrameField v-if="entry.mode === 'frame'" label="Frame"><FrameNumberField v-model="entry.frame" label="Frame" controls="stacked" :min="1" :max="count" :step="1" :disabled="disabled" required /></FrameField>
      <FrameField label="Scale (nearest neighbor)"><FrameNumberField v-model="entry.scale" label="Scale (nearest neighbor)" controls="stacked" :min="1" :max="64" :step="1" :disabled="disabled" required /></FrameField>
    </div>
    <div v-if="entry.mode === 'sheet'" class="export-field-pair">
      <FrameField label="Arrange by"><FrameSelect v-model="entry.gridAxis" :label="`Arrange by ${index + 1}`" :options="[{value:'columns',label:'Columns'},{value:'rows',label:'Rows'}]" :disabled="disabled" /></FrameField>
      <FrameField :label="entry.gridAxis === 'columns' ? 'Columns' : 'Rows'"><FrameNumberField v-model="entry.gridCount" :label="entry.gridAxis === 'columns' ? 'Columns' : 'Rows'" controls="stacked" :min="1" :max="count" :step="1" :disabled="disabled" required /></FrameField>
    </div>
    <p v-if="entry.mode === 'sheet'" class="hint">{{ size.columns }} columns × {{ size.rows }} rows, in timeline order.</p>
    <div class="export-background">
      <FrameCheckbox :model-value="backgroundVisible" label="Use background color" :disabled="disabled || needsBackground(entry.format)" @update:model-value="entry.opaque = $event" />
      <span class="export-background-color" :class="{ 'is-hidden': !backgroundVisible }" :inert="!backgroundVisible || undefined" :aria-hidden="!backgroundVisible">
        <FrameColorPicker v-model="entry.background" aria-label="Background color" :disabled="disabled || !backgroundVisible" />
      </span>
    </div>
    <p v-if="entry.format === 'gif'" class="hint">GIF uses binary transparency: alpha below 50% becomes transparent; higher alpha becomes opaque. Use a background color to flatten transparency.</p>
    <FrameCheckbox v-if="entry.mode === 'animation' && count > 1" v-model="entry.loopAnimation" label="Loop animation" :disabled="disabled" />
    <p v-if="entry.mode === 'video'" class="hint">Video requires FFmpeg on your PATH and an opaque background.</p>
  </fieldset>
  <p v-else class="hint">Includes every editable frame, layer and palette color.</p>
  <div class="export-output-summary">
    <span class="export-filename">{{ filename }}</span>
    <span v-if="entry.mode !== 'project'" class="hint export-pixel-size">{{ size.width }} × {{ size.height }} pixels</span>
  </div>
  <p v-if="!valid" role="alert">Check the frame, grid and scale values. Output is limited to 4096 pixels per side and 4,194,304 pixels.</p>
</section>
</template>
<style scoped>
.export-entry { display: flex; flex-direction: column; gap: 12px; border: 1px solid var(--frame-border-color); border-radius: 6px; padding: 16px; }
.export-entry p { margin: 0; }
.export-entry-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.export-field-pair { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }
.export-entry-fields { display: flex; flex-direction: column; gap: 12px; padding: 0; border: 0; margin: 0; min-width: 0; }
.export-background { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.export-background-color { display: inline-flex; flex: none; }
.is-hidden { visibility: hidden; }
.export-output-summary { display: flex; align-items: baseline; justify-content: space-between; gap: 16px; }
.export-filename { overflow-wrap: anywhere; min-width: 0; font-family: monospace; }
.export-pixel-size { flex: none; text-align: right; }
@media (max-width: 480px) { .export-field-pair { grid-template-columns: 1fr; } }
</style>
