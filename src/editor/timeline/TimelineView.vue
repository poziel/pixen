<script setup lang="ts">
import { computed } from "vue";
import { useEditor } from "../editor";
import Thumbnail from "../components/Thumbnail.vue";
import PreviewRow from "../components/PreviewRow.vue";
const editor = useEditor(), s = editor.state;
const doc = computed(() => s.snapshot?.document);
</script>
<template>
  <div class="preview-list">
    <p v-if="!doc" class="empty-note">Open animated artwork to see its timeline.</p>
    <PreviewRow v-for="(frame, index) in doc?.frames" :key="frame.id"
      :name="`Frame ${index + 1}`" :subtitle="`${frame.duration_ms} ms`"
      :selected="index === s.frame" :disabled="editor.following()" @select="editor.select(index)">
      <template #preview><Thumbnail :frame-id="frame.id" /></template>
    </PreviewRow>
  </div>
</template>
