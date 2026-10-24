<script setup lang="ts">
import { useEditor } from '../editor';
import { tools } from './raster';
import ColorPicker from './ColorPicker.vue';
import { computed } from 'vue';
const editor = useEditor(), s = editor.state, t = s.tools;
const activeLeft = computed(() => t.temporaryPan ? 'pan' : t.left);
function history(redo: boolean) { editor.action('edit_history', { documentId: s.snapshot?.active_document, redo }); }
</script>
<template>
  <div class="pixel-tools sidebar-content">
    <div class="tool-heading"><strong>Tools</strong><button :disabled="s.working||!s.snapshot?.document||s.snapshot?.managed" @click="history(false)">Undo</button><button :disabled="s.working||!s.snapshot?.document||s.snapshot?.managed" @click="history(true)">Redo</button></div>
    <p class="hint">Click to assign left · Right-click to assign right</p>
    <div class="tool-grid" role="group" aria-label="Drawing tools">
      <button v-for="tool in tools" :key="tool.id" :title="tool.label" :class="{'assigned-left':activeLeft===tool.id,'assigned-right':t.right===tool.id}" :aria-label="`${tool.label}${activeLeft===tool.id?' (left button)':''}${t.right===tool.id?' (right button)':''}`" :aria-pressed="activeLeft===tool.id||t.right===tool.id" @click="t.left=tool.id" @contextmenu.prevent="t.right=tool.id"><span aria-hidden="true">{{tool.glyph}}</span><small v-if="activeLeft===tool.id||t.right===tool.id">{{ activeLeft===tool.id?'L':'' }}{{ t.right===tool.id?'R':'' }}</small></button>
    </div>
    <label>Left button<select v-model="t.left" aria-label="Left button"><option v-for="tool in tools" :key="tool.id" :value="tool.id">{{tool.label}}</option></select></label>
    <label>Right button<select v-model="t.right" aria-label="Right button"><option v-for="tool in tools" :key="tool.id" :value="tool.id">{{tool.label}}</option></select></label>
    <ColorPicker />
    <section v-for="slot in (['left','right'] as const)" :key="slot" class="tool-settings" :aria-label="`${slot} tool settings`">
      <h3 :class="`tool-slot-${slot}`">{{slot==='left'?'Left':'Right'}} · {{tools.find(tool=>tool.id===t[slot])?.label}}</h3>
      <template v-if="t[slot]==='pencil'||t[slot]==='eraser'">
        <label v-for="setting in (['size','density','opacity'] as const)" :key="setting">{{setting==='size'?'Size (px)':setting==='density'?'Density (%)':'Opacity (%)'}}<input v-model.number="t.brushes[slot][t[slot] as 'pencil'|'eraser'][setting]" type="number" :min="setting==='size'?1:0" :max="setting==='size'?256:100" /></label>
      </template>
      <template v-if="['rectangle','ellipse','polygon','lasso','color'].includes(t[slot])"><label>Selection mode<select v-model="t.selectionMode"><option value="replace">Replace</option><option value="add">Add</option><option value="subtract">Subtract</option></select></label><button @click="s.selectionClear++">Deselect</button><p class="hint">Polygon: click vertices, double-click to finish. Escape cancels.</p></template>
      <template v-if="t[slot]==='bucket'||t[slot]==='color'"><label>Tolerance<input v-model.number="t.tolerance" type="number" min="0" max="255" /></label><label><input v-model="t.contiguous" type="checkbox" /> Contiguous pixels</label></template>
      <label v-if="t[slot]==='move'"><input v-model="t.moveCopy" type="checkbox" /> Copy when moving</label>
      <template v-if="t[slot]==='text'"><label>Text<input v-model="t.text" maxlength="512" /></label><label>Size (px)<input v-model.number="t.textSize" type="number" min="1" max="128" /></label></template>
      <p v-if="t[slot]==='paper'" class="hint">Fill the active layer with this button’s color, inside the selection.</p>
      <template v-if="t[slot]==='zoom'"><button @click="s.zoomReset++">Fit artwork</button><button @click="s.zoomOne++">100%</button><p class="hint">Click to zoom in. Right-click or Alt-click to zoom out.</p></template>
    </section>
    <p class="hint">Blue: left · Orange: right · X swaps colors · Hold Space or Enter to pan · Ctrl+wheel sizes left · Ctrl+Shift+wheel sizes right · Ctrl+Z undoes · Escape deselects</p>
    <p v-if="s.snapshot?.managed" class="hint">Managed preview is read-only.</p>
  </div>
</template>



