<script setup lang="ts">
import { computed } from "vue";
import { FrameButton } from "@frame/core";
import { useEditor } from "./editor";
import Thumbnail from "./Thumbnail.vue";
const editor = useEditor(),
  s = editor.state;
const doc = computed(() => s.snapshot?.document);
const frame = computed(() => doc.value?.frames[s.frame]?.id ?? "");
function toggle(id: string) {
  s.hidden = s.hidden.includes(id)
    ? s.hidden.filter((x) => x !== id)
    : [...s.hidden, id];
}
function solo(id: string) {
  s.hidden =
    doc.value?.layers.filter((l) => l.id !== id).map((l) => l.id) ?? [];
}
function reset() {
  s.hidden = doc.value?.layers.filter((l) => !l.visible).map((l) => l.id) ?? [];
}
</script>
<template>
  <div class="sidebar-content">
    <div class="view-tools">
      <span>{{ doc?.layers.length ?? 0 }} layers</span
      ><FrameButton
        label="Reset view"
        variant="ghost"
        :disabled="editor.following()"
        @click="reset"
      />
    </div>
    <p v-if="!doc" class="empty-note">
      Open or create artwork to see its layers.
    </p>
    <div
      v-for="layer in [...(doc?.layers ?? [])].reverse()"
      :key="layer.id"
      class="layer-row"
      :class="{ selected: s.layer === layer.id }"
    >
      <fieldset class="layer-tools" :disabled="editor.following()">
        <button
          :aria-label="`${s.hidden.includes(layer.id) ? 'Show' : 'Hide'} ${layer.name}`"
          :aria-pressed="!s.hidden.includes(layer.id)"
          @click="toggle(layer.id)"
        >
          {{ s.hidden.includes(layer.id) ? "○" : "●" }}</button
        ><button
          :aria-label="`Solo ${layer.name}`"
          title="Show only this layer"
          @click="solo(layer.id)"
        >
          ◎
        </button>
      </fieldset>
      <button class="layer-select" @click="s.layer = layer.id">
        <Thumbnail :frame-id="frame" :layer-id="layer.id" /><span
          ><strong>{{ layer.name }}</strong
          ><small
            >{{ Math.round(layer.opacity * 100) }}% opacity{{
              !layer.visible ? " · hidden in document" : ""
            }}</small
          ></span
        >
      </button>
    </div>
  </div>
</template>
