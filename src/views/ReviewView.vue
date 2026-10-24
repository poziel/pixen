<script setup lang="ts">
import { useEditor } from "../editor/editor";
const editor = useEditor(),
  s = editor.state;
function compare(from: string, to: string) {
  const doc = s.snapshot?.document,
    index = doc?.frames.findIndex((f) => f.id === to) ?? -1;
  if (index < 0) return;
  editor.select(index);
  s.mode = "frame_comparison";
  s.reference = from;
}
</script>
<template>
  <div class="dialog-content review-content" v-if="s.review">
    <p>
      {{ s.review.report.kind ?? "Art review" }} ·
      {{ s.review.report.inspected_revision ?? s.snapshot?.document?.revision }}
    </p>
    <p class="hint">Read-only evidence. Artwork stays unchanged.</p>
    <label v-if="s.review.report.components" class="field"
      ><input v-model="s.annotations" type="checkbox" />Component bounds</label
    >
    <div
      v-for="(pair, index) in s.review.report.transitions ?? []"
      :key="index"
    >
      <button @click="compare(pair.from_frame_id, pair.to_frame_id)">
        Compare {{ pair.from_frame_id }} → {{ pair.to_frame_id }}
      </button>
    </div>
    <figure v-for="image in s.review.images" :key="image.role">
      <figcaption>{{ image.role }}</figcaption>
      <img :src="`data:image/png;base64,${image.png}`" :alt="image.role" />
    </figure>
    <details>
      <summary>Report details</summary>
      <pre>{{ JSON.stringify(s.review.report, null, 2) }}</pre>
    </details>
  </div>
</template>
