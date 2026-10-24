<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { FrameFileDropZone } from '@frame/core';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEditor } from '../editor';

const editor = useEditor();
const host = ref<HTMLElement>();
const zone = ref<InstanceType<typeof FrameFileDropZone>>();
const paths = new WeakMap<File, string>();
let unlisten: (() => void) | undefined;
let disposed = false;
let inside = false;

async function accepted(files: File[]) {
  if (editor.state.snapshot?.managed) return;
  for (const file of files) {
    const path = paths.get(file);
    if (path) await editor.action('open_document', { path });
  }
}

// Tauri intercepts OS drops, so forward them through Frame's drag and
// acceptance lifecycle while retaining native paths for document opening.
onMounted(async () => {
  if (!isTauri()) return;
  const window = getCurrentWindow();
  const stop = await window.onDragDropEvent(({ payload }) => {
    if (disposed) return;
    const target = host.value?.querySelector('.frame-file-drop-zone');
    if (!target) return;
    if (payload.type === 'leave' || editor.state.snapshot?.managed) {
      inside = false;
      zone.value?.reset();
      return;
    }
    const factor = globalThis.devicePixelRatio || 1;
    const bounds = target.getBoundingClientRect();
    const x = payload.position.x / factor, y = payload.position.y / factor;
    const over = x >= bounds.left && x < bounds.right && y >= bounds.top && y < bounds.bottom;
    if (!over) {
      inside = false;
      zone.value?.reset();
      return;
    }
    const transfer = new DataTransfer();
    for (const path of 'paths' in payload ? payload.paths : ['']) {
      const file = new File([], path.split(/[\\/]/).pop() || 'file');
      paths.set(file, path);
      transfer.items.add(file);
    }
    const type = payload.type === 'drop' ? 'drop' : inside ? 'dragover' : 'dragenter';
    inside = payload.type !== 'drop';
    target.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: transfer }));
  });
  if (disposed) stop();
  else unlisten = stop;
});
onBeforeUnmount(() => { disposed = true; unlisten?.(); });
</script>

<template>
  <div ref="host" class="artwork-drop-host">
    <FrameFileDropZone ref="zone" class="artwork-drop-zone" appearance="overlay" multiple
      :disabled="!!editor.state.snapshot?.managed" message="Drop files to open artwork" @accepted="accepted">
      <slot />
    </FrameFileDropZone>
  </div>
</template>
