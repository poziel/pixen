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
  if (!valid.value) return;
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
    class="dialog-content"
    @submit.prevent="submit"
  >
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
    <p class="hint">
      {{
        kind === "resize"
          ? "Pixels are anchored at the top left. Cancel restores the original canvas."
          : "Create a transparent canvas."
      }}
    </p>
    <p v-if="s.error" role="alert">{{ s.error }}</p>
    <div class="dialog-actions">
      <FrameButton
        label="Cancel"
        @click="frame.closeModal('pixloom.dialog')"
      /><FrameButton
        :label="kind === 'new' ? 'Create' : 'Resize canvas'"
        type="submit"
        variant="primary"
        :disabled="!valid || s.working"
      />
    </div>
  </form>
  <div v-else class="dialog-content">
    <h3>Playback</h3>
    <label class="field"
      >Preview speed
      <input
        v-model.number="s.speed"
        type="range"
        min="0.125"
        max="8"
        step="0.125"
      /><span>{{ s.speed }}×</span></label
    >
    <p class="hint">Saved frame timing stays unchanged.</p>
    <h3>Canvas</h3>
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
