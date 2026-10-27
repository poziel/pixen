<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useEditor } from "../editor";
import { zoomPan } from "./zoom";
import { LatestRequest } from "../timeline/playback";
import ArtworkDropZone from './ArtworkDropZone.vue';
import { useTools } from '../tools/gestures';
import ToolCursor from '../tools/ToolCursor.vue';
import SelectionOverlay from '../tools/SelectionOverlay.vue';
import { resizedToolValue } from '../tools/interaction';
import { isShape } from '../tools/shapes';
import type { Point } from '../tools/raster';


const editor = useEditor(),
  s = editor.state;
const stage = ref<HTMLElement>(),
  canvas = ref<HTMLCanvasElement>();
const width = ref(s.canvasWidth || 1),
  height = ref(s.canvasHeight || 1),
  viewport = ref({ width: 1, height: 1 }),
  zoom = computed({ get: () => s.canvasView.zoom, set: value => { s.canvasView.zoom = value; } }),
  pan = computed({ get: () => s.canvasView.pan, set: value => { s.canvasView.pan = value; } });
const pending = new LatestRequest();
const pointer = ref<Point|null>(null), pointerButtons = ref(0);
const cursorPixel = computed(() => {
  if (!pointer.value || !stage.value || manual.panHeld.value || s.tools.globalPicker || s.tools.temporaryPicker) return null;
  const bounds = stage.value.getBoundingClientRect();
  const x = Math.floor((pointer.value.x-bounds.left-viewport.value.width/2-pan.value.x)/scale.value+width.value/2);
  const y = Math.floor((pointer.value.y-bounds.top-viewport.value.height/2-pan.value.y)/scale.value+height.value/2);
  return {x,y};
});
let observer: ResizeObserver | undefined;
let focusFrame=0;
watch(()=>s.canvasFocus,()=>{
  cancelAnimationFrame(focusFrame);
  // Let menu dismissal finish restoring its trigger before returning to artwork.
  focusFrame=requestAnimationFrame(()=>{
    const active=document.activeElement;
    if(!s.snapshot?.document||s.dialog||active?.closest('input,textarea,select,[contenteditable=true],[role=dialog]'))return;
    if(document.querySelector('[role=menu]:not([data-state=closed]),[role=listbox]:not([data-state=closed]),[role=dialog]'))return;
    stage.value?.focus({preventScroll:true});
  });
});
const doc = computed(() => s.snapshot?.document);
const fitScale = computed(() => Math.max(0.01, Math.min(
  (viewport.value.width - 48) / width.value,
  (viewport.value.height - 48) / height.value,
)));
const scale = computed(() => s.canvasView.fit ? fitScale.value : zoom.value);
const checkerCell = computed(() =>
  scale.value < 1
    ? 16
    : scale.value < 2
      ? 8
      : scale.value < 4
        ? 4
        : scale.value < 8
          ? 2
          : 1,
);
const style = computed(() => ({
  width: `${width.value}px`,
  height: `${height.value}px`,
  backgroundSize: `${2 * checkerCell.value}px ${2 * checkerCell.value}px`,
  boxShadow: `0 0 0 ${1 / scale.value}px #444`,
  transform: `translate(calc(-50% + ${pan.value.x}px),calc(-50% + ${pan.value.y}px)) scale(${scale.value})`,
}));
watch(
  () => JSON.stringify([s.snapshot?.active_document, editor.request()]),
  async () => {
    const serial = pending.begin(),
      request = editor.request();
    if (!request) return;
    try {
      const image = await editor.image(request),
        bitmap = new Image();
      bitmap.src = `data:image/png;base64,${image.png}`;
      await bitmap.decode();
      if (!pending.current(serial) || !canvas.value) return;
      width.value = image.width;
      height.value = image.height;
      s.canvasWidth = image.width;
      s.canvasHeight = image.height;
      canvas.value.width = image.width;
      canvas.value.height = image.height;
      const context = canvas.value.getContext("2d")!;
      context.imageSmoothingEnabled = false;
      context.clearRect(0, 0, image.width, image.height);
      context.drawImage(bitmap, 0, 0);
      manual.paintSelection();
    } catch (e) {
      if (pending.current(serial) && !s.working) s.error = String(e);
    }
  },
  { immediate: true, flush: "post" },
);
function reset() {
  s.canvasView.fit = true;
  zoom.value = 1;
  pan.value = { x: 0, y: 0 };
}
watch(() => s.zoomReset, reset);
watch(
  () => s.zoomOne,
  () => {
    s.canvasView.fit = false;
    zoom.value = 1;
    pan.value = { x: 0, y: 0 };
  },
);
watch(scale, (value) => (s.canvasZoom = Math.round(value * 100)), {
  immediate: true,
});

function setScale(next: number, anchor = { x: 0, y: 0 }) {
  const previous = scale.value;
  const target = Math.min(256, Math.max(0.01, next));
  pan.value = zoomPan(pan.value, anchor, target / previous);
  zoom.value = target;
  s.canvasView.fit = false;
}
watch(() => s.zoomRequest, request => {
  if (request.scale !== undefined) setScale(request.scale);
  else if (request.factor !== undefined) setScale(scale.value * request.factor);
});
function wheel(e: WheelEvent) {
  const bounds = stage.value?.getBoundingClientRect();
  if (!bounds || !doc.value) return;
  pointer.value = {x:e.clientX,y:e.clientY};
  if (e.ctrlKey) {
    const slot = e.shiftKey || (pointerButtons.value&2) ? 'right' : 'left';
    const tool = s.tools[slot];
    const brush = tool==='pencil'||tool==='eraser'?s.tools.brushes[slot][tool]:isShape(tool)?s.tools.options[slot].shapes[tool]:tool==='paint-select'?s.tools.options[slot].selectionBrush:null;
    const size = resizedToolValue(tool,brush?.size??s.tools.options[slot].text.size,e.deltaY);
    if (size!==null) {
      if (brush) brush.size=size;
      else s.tools.options[slot].text.size=size;
    }
    return;
  }
  setScale(scale.value * (e.deltaY < 0 ? 1.15 : 1 / 1.15), {
    x: e.clientX - bounds.left - bounds.width / 2,
    y: e.clientY - bounds.top - bounds.height / 2,
  });
}
let drag: { x: number; y: number; px: number; py: number } | null = null;
const manual = useTools(editor,stage,canvas,setScale,scale);
function down(e: PointerEvent) {
  pointer.value = {x:e.clientX,y:e.clientY};
  pointerButtons.value=e.buttons;
  const tool=e.button===2?s.tools.right:s.tools.left;
  if(e.button!==1&&!manual.panHeld.value&&tool!=='pan'){void manual.down(e);return;}
  stage.value?.setPointerCapture(e.pointerId);
  drag = { x: e.clientX, y: e.clientY, px: pan.value.x, py: pan.value.y };
}
function move(e: PointerEvent) {
  pointer.value = {x:e.clientX,y:e.clientY};
  pointerButtons.value=e.buttons;
  if (!drag) manual.move(e);
  if (drag)
    pan.value = {
      x: drag.px + e.clientX - drag.x,
      y: drag.py + e.clientY - drag.y,
    };
}
onMounted(() => {
  observer = new ResizeObserver(
    ([entry]) =>
      (viewport.value = {
        width: entry.contentRect.width,
        height: entry.contentRect.height,
      }),
  );
  if (stage.value) observer.observe(stage.value);
});
onBeforeUnmount(() => {
  cancelAnimationFrame(focusFrame);
  observer?.disconnect();
  pending.begin();
});
</script>
<template>
  <ArtworkDropZone>
  <div class="canvas-workspace">
    <div v-if="s.error" class="error-banner" role="alert">
      <span>{{ s.error }}</span
      ><button aria-label="Dismiss error" @click="s.error = ''">×</button>
    </div>
    <div
      ref="stage"
      class="canvas-stage"
      :class="{ empty: !doc }"
      tabindex="0"
      aria-label="Artwork canvas. Draw with left or right button. Hold Space or Enter to pan. Ctrl+wheel resizes tools. Scroll to zoom."
      :style="{cursor:manual.panHeld.value||s.tools.left==='pan'?'grab':manual.selectionCursor.value??'crosshair'}"
      @contextmenu.prevent
      @wheel.prevent="wheel"
      @pointerdown="down"
      @pointermove="move"
      @pointerup="drag = null; pointerButtons=$event.buttons; manual.up()"
      @pointercancel="drag = null; pointerButtons=0; pointer=null; manual.cancel()"
      @pointerleave="pointer=null"
      @dblclick="manual.doubleClick()"
    >
      <canvas
        v-show="doc"
        ref="canvas"
        :style="style"
        class="artwork"
        :class="[`background-${s.background}`, { checkerboard: s.background === 'checkerboard' }]"
        aria-label="Pixel artwork"
      />
      <SelectionOverlay v-if="doc" :width="width" :height="height" :scale="scale" :style="style" />
      <ToolCursor v-if="doc" :pixel="cursorPixel" :width="width" :height="height" :scale="scale" :style="style" />
    </div>
  </div>
  </ArtworkDropZone>
</template>
