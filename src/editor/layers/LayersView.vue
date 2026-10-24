<script setup lang="ts">
import { computed } from "vue";
import { FrameIconButton } from "@frame/core";
import PreviewRow from "../components/PreviewRow.vue";
import { useEditor } from "../editor";
import Thumbnail from "../components/Thumbnail.vue";
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
</script>
<template>
  <div class="preview-list">
    <p v-if="!doc" class="empty-note">Open or create artwork to see its layers.</p>
    <PreviewRow v-for="layer in [...(doc?.layers ?? [])].reverse()" :key="layer.id"
      :name="layer.name" :subtitle="`${Math.round(layer.opacity * 100)}% opacity${!layer.visible ? ' · hidden in document' : ''}`"
      :selected="s.layer === layer.id" @select="s.layer = layer.id">
      <template #actions>
        <FrameIconButton variant="icon" :size="24"
          :label="`${s.hidden.includes(layer.id) ? 'Show' : 'Hide'} ${layer.name}`"
          :aria-pressed="!s.hidden.includes(layer.id)"
          :icon="s.hidden.includes(layer.id) ? 'codicon:eye-closed' : 'codicon:eye'"
          :disabled="editor.following()" @click="toggle(layer.id)" />
        <FrameIconButton variant="icon" :size="24" :label="`Solo ${layer.name}`"
          tooltip="Show only this layer" icon="codicon:circle-filled"
          :disabled="editor.following()" @click="solo(layer.id)" />
      </template>
      <template #preview><Thumbnail :frame-id="frame" :layer-id="layer.id" /></template>
    </PreviewRow>
  </div>
</template>

