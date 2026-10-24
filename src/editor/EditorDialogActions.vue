<script setup lang="ts">
import { FrameButton, useFrame } from '@frame/core';
import { useEditor } from './editor';
defineProps<{ kind: 'new' | 'resize' }>();
const frame = useFrame();
const { state } = useEditor();
</script>
<template>
  <FrameButton label="Cancel" @click="frame.closeModal('pixloom.dialog')" />
  <FrameButton :label="kind === 'new' ? 'Create' : 'Resize canvas'" type="submit"
    form="pixloom-dimensions" variant="primary" :disabled="state.working" />
</template>
