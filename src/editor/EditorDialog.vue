<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { FrameButton, useFrame } from "@frame/core";
import { useEditor } from "./editor";
const props = defineProps<{ kind: "new" | "resize" | "settings" }>();
const editor = useEditor(),
  s = editor.state,
  frame = useFrame();
const width = ref(
    props.kind === "new" ? 32 : (s.snapshot?.document?.source_width ?? 32),
  ),
  height = ref(
    props.kind === "new" ? 32 : (s.snapshot?.document?.source_height ?? 32),
  );
const valid = computed(
  () =>
    Number.isInteger(width.value) &&
    Number.isInteger(height.value) &&
    width.value > 0 &&
    height.value > 0 &&
    width.value * height.value <= 16 * 1024 * 1024,
);
const sizes = [8, 16, 24, 32, 48, 64, 96, 128, 256];
function chooseSize(size: number) { width.value = size; height.value = size; }
watch(
  [width, height],
  () => {
    if (props.kind === "resize")
      s.previewSize = valid.value
        ? { width: width.value, height: height.value }
        : null;
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  s.previewSize = null;
  s.dialog = "";
});
async function submit() {
  if (!valid.value || s.working) return;
  await editor.action(
    props.kind === "new" ? "create_document" : "resize_document",
    { width: width.value, height: height.value },
  );
  if (!s.error) frame.closeModal("pixloom.dialog");
}
</script>
<template>
  <form
    v-if="kind !== 'settings'"
    id="pixloom-dimensions"
    class="dialog-content"
    @submit.prevent="submit"
  >
    <fieldset v-if="kind === 'new'" class="artwork-presets">
      <legend>Quick sizes</legend>
      <div class="artwork-size-grid">
        <FrameButton v-for="size in sizes" :key="size" :label="`${size} × ${size}`"
          :aria-pressed="width === size && height === size"
          :variant="width === size && height === size ? 'primary' : 'secondary'"
          @click="chooseSize(size)" />
      </div>
    </fieldset>
    <div class="artwork-dimensions">
    <label class="field"
      >Width
      <input
        v-model.number="width"
        type="number"
        min="1"
        max="16384"
        required
        autofocus
    /></label>
    <label class="field"
      >Height
      <input v-model.number="height" type="number" min="1" max="16384" required
    /></label>
    </div>
    <p v-if="!valid" role="alert">Enter positive whole dimensions totaling at most 16,777,216 pixels.</p>
    <p class="hint">
      {{
        kind === "resize"
          ? "Pixels are anchored at the top left. Cancel restores the original canvas."
          : "Choose a quick size or enter custom dimensions in pixels."
      }}
    </p>
    <p v-if="s.error" role="alert">{{ s.error }}</p>
  </form>
  <div v-else class="dialog-content">
    <h3>Playback</h3>
    <label class="field"
      >Preview speed
      <select v-model.number="s.fps" @change="s.elapsed = 0">
        <option v-for="fps in [...new Set([0.5, 1, 3, 5, 8, 15, 20, s.fps])].sort((a, b) => a - b)" :key="fps" :value="fps">{{ fps }} FPS</option>
      </select></label
    >
    <p class="hint">Saved frame timing stays unchanged.</p>
    <h3>Canvas</h3>
    <label v-if="s.mode !== 'composite'" class="field"
      >Comparison reference
      <select v-model="s.reference">
        <option :value="null">Neighboring frames</option>
        <option
          v-for="(item, index) in s.snapshot?.document?.frames"
          :key="item.id"
          :value="item.id"
        >
          Frame {{ index + 1 }}
        </option>
      </select></label
    >
    <label class="field"
      >Background
      <select v-model="s.background">
        <option value="checkerboard">Pixel checkerboard</option>
        <option value="transparent">Transparent</option>
        <option value="black">Black</option>
        <option value="white">White</option>
      </select></label
    >
    <label class="field"
      >Color mode
      <select v-model="s.representation">
        <option value="color">Color</option>
        <option value="silhouette">Silhouette</option>
        <option value="grayscale">Grayscale</option>
      </select></label
    >
    <label v-if="s.snapshot?.managed" class="field"
      ><input v-model="s.follow" type="checkbox" />Follow MCP activity</label
    >
    <h3>Workspace</h3>
    <p class="hint">
      Layers, Timeline, and AI chat are available from the primary activity bar.
    </p>
    <FrameButton
      label="Done"
      variant="primary"
      @click="frame.closeModal('pixloom.dialog')"
    />
  </div>
</template>
