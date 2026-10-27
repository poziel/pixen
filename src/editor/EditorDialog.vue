<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { FrameButton, useFrame } from "@frame/core";
import { useEditor } from "./editor";
import SettingsView from '../views/SettingsView.vue';
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
  if (!s.error) frame.closeModal("pixen.dialog");
}
</script>
<template>
  <form
    v-if="kind !== 'settings'"
    id="pixen-dimensions"
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
  <SettingsView v-else />
</template>
