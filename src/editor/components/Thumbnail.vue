<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import { useEditor } from "../editor";
import { LatestRequest } from "../timeline/playback";
let owner: string | null | undefined;
const props = defineProps<{ frameId: string; layerId?: string }>();
const editor = useEditor(),
  src = ref(""),
  pending = new LatestRequest();
watch(
  () => [
    editor.state.snapshot?.active_document,
    editor.state.snapshot?.document?.generation,
    props.frameId,
    props.layerId,
  ],
  async () => {
    const serial = pending.begin(),
      doc = editor.state.snapshot?.document;
    const documentId = editor.state.snapshot?.active_document;
    if (documentId !== owner || !doc) src.value = "";
    owner = documentId;
    if (!doc) return;
    try {
      const image = await editor.image({
        generation: doc.generation,
        frame_id: props.frameId,
        layer_id: props.layerId,
      });
      if (!pending.current(serial)) return;
      const nextSrc = `data:image/png;base64,${image.png}`;
      const decoded = new Image();
      decoded.src = nextSrc;
      await decoded.decode();
      if (pending.current(serial)) src.value = nextSrc;
    } catch {
      /* Next document refresh retries. */
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => pending.begin());
</script>
<template>
  <span class="thumbnail checkerboard"
    ><img v-if="src" :src="src" alt="" draggable="false"
  /></span>
</template>
