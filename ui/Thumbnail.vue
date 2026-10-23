<script setup lang="ts">
import { ref, watch } from "vue";
import { useEditor } from "./editor";
import { LatestRequest } from "./playback";
const props = defineProps<{ frameId: string; layerId?: string }>();
const editor = useEditor(),
  src = ref(""),
  pending = new LatestRequest();
watch(
  () => [
    editor.state.snapshot?.document?.generation,
    props.frameId,
    props.layerId,
  ],
  async () => {
    const serial = pending.begin(),
      doc = editor.state.snapshot?.document;
    src.value = "";
    if (!doc) return;
    try {
      const image = await editor.image({
        generation: doc.generation,
        frame_id: props.frameId,
        layer_id: props.layerId,
      });
      if (pending.current(serial))
        src.value = `data:image/png;base64,${image.png}`;
    } catch {
      /* Next document refresh retries. */
    }
  },
  { immediate: true },
);
</script>
<template>
  <span class="thumbnail checkerboard"
    ><img v-if="src" :src="src" alt="" draggable="false"
  /></span>
</template>
