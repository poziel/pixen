<script setup lang="ts">
import { FrameButton, useFrame } from '@frame/core';
import { useEditor } from './editor';
const frame = useFrame();
const { state } = useEditor();
</script>
<template>
  <FrameButton label="Cancel" :disabled="state.working" @click="frame.closeModal('pixen.dialog')" />
  <FrameButton :label="state.working ? 'Exporting…' : 'Export'" type="submit" form="pixen-export"
    variant="primary" :disabled="state.working || !state.exportValid" />
</template>
