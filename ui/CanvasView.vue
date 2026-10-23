<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FrameButton } from "@frame/core";
import { useEditor } from "./editor";
import { LatestRequest } from "./playback";
import emptyIcon from "../assets/icons/empty-workspace.svg";
const editor = useEditor(),
  s = editor.state;
const stage = ref<HTMLElement>(),
  canvas = ref<HTMLCanvasElement>();
const width = ref(1),
  height = ref(1),
  viewport = ref({ width: 1, height: 1 }),
  zoom = ref(1),
  pan = ref({ x: 0, y: 0 });
const pending = new LatestRequest();
let observer: ResizeObserver | undefined;
const doc = computed(() => s.snapshot?.document);
const scale = computed(
  () =>
    Math.max(
      0.01,
      Math.min(
        (viewport.value.width - 48) / width.value,
        (viewport.value.height - 48) / height.value,
      ),
    ) * zoom.value,
);
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
  () => JSON.stringify(editor.request()),
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
      canvas.value.width = image.width;
      canvas.value.height = image.height;
      const context = canvas.value.getContext("2d")!;
      context.imageSmoothingEnabled = false;
      context.clearRect(0, 0, image.width, image.height);
      context.drawImage(bitmap, 0, 0);
    } catch (e) {
      if (pending.current(serial)) s.error = String(e);
    }
  },
  { immediate: true, flush: "post" },
);
function reset() {
  zoom.value = 1;
  pan.value = { x: 0, y: 0 };
}
watch(() => s.zoomReset, reset);
watch(() => doc.value?.path, reset);
function wheel(e: WheelEvent) {
  zoom.value = Math.min(
    128,
    Math.max(0.05, zoom.value * (e.deltaY < 0 ? 1.15 : 1 / 1.15)),
  );
}
let drag: { x: number; y: number; px: number; py: number } | null = null;
function down(e: PointerEvent) {
  if (e.button !== 0 && e.button !== 1) return;
  stage.value?.setPointerCapture(e.pointerId);
  drag = { x: e.clientX, y: e.clientY, px: pan.value.x, py: pan.value.y };
}
function move(e: PointerEvent) {
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
  observer?.disconnect();
  pending.begin();
});
</script>
<template>
  <div class="canvas-workspace">
    <div v-if="s.error" class="error-banner" role="alert">
      <span>{{ s.error }}</span
      ><button aria-label="Dismiss error" @click="s.error = ''">×</button>
    </div>
    <div
      ref="stage"
      class="canvas-stage"
      :class="[`background-${s.background}`, { empty: !doc }]"
      tabindex="0"
      aria-label="Artwork canvas. Scroll to zoom, drag to pan."
      @wheel.prevent="wheel"
      @pointerdown="down"
      @pointermove="move"
      @pointerup="drag = null"
      @pointercancel="drag = null"
      @dblclick="reset"
    >
      <div v-if="!doc" class="empty-workspace">
        <img :src="emptyIcon" alt="" />
        <h1>Pixloom</h1>
        <p>Open pixel art or start a new canvas.</p>
        <div @pointerdown.stop>
          <FrameButton
            label="Open artwork"
            @click="editor.action('open_document')"
          /><FrameButton
            label="New artwork"
            variant="primary"
            @click="s.dialog = 'new'"
          />
        </div>
      </div>
      <canvas
        v-show="doc"
        ref="canvas"
        :style="style"
        class="artwork"
        :class="{ checkerboard: s.background === 'checkerboard' }"
        aria-label="Pixel artwork"
      />
    </div>
    <div v-if="doc" class="canvas-tools">
      <span
        >{{ width }} × {{ height }} · Frame {{ s.frame + 1 }} /
        {{ doc.frames.length }}</span
      ><label v-if="s.mode !== 'composite'"
        >Reference
        <select v-model="s.reference">
          <option :value="null">Neighboring frames</option>
          <option
            v-for="(frame, i) in doc.frames"
            :key="frame.id"
            :value="frame.id"
          >
            Frame {{ i + 1 }}
          </option>
        </select></label
      ><button v-if="s.isolatedLayer" @click="s.isolatedLayer = null">
        Show all layers</button
      ><button @click="reset">Fit</button
      ><button
        @click="
          zoom =
            1 /
            Math.max(
              0.01,
              Math.min(
                (viewport.width - 48) / width,
                (viewport.height - 48) / height,
              ),
            );
          pan = { x: 0, y: 0 };
        "
      >
        100%</button
      ><span>{{ Math.round(scale * 100) }}%</span>
    </div>
  </div>
</template>
