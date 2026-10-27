<script setup lang="ts">
import { computed, defineComponent, h, markRaw } from "vue";
import { FrameIconButton, useFrame } from "@frame/core";
import { useEditor } from "../editor";
const editor = useEditor(), s = editor.state;
const frame = useFrame();
const doc = computed(() => s.snapshot?.document);
const boundaryIcon = (last: boolean) => markRaw(defineComponent(() => () =>
  h('svg', { viewBox: '0 0 16 16', fill: 'none', stroke: 'currentColor', 'stroke-width': 1, 'aria-hidden': 'true' }, [
    h('path', { d: last ? 'M11.5 3v10M5 3.5 9.5 8 5 12.5' : 'M4.5 3v10M11 3.5 6.5 8 11 12.5' }),
  ]),
));
const firstIcon = boundaryIcon(false), lastIcon = boundaryIcon(true);
</script>
<template>
  <fieldset :disabled="editor.following()" style="border:0;margin:0;padding:0">
    <div class="timeline-playback-tools">
      <FrameIconButton label="First Frame" shortcut="Ctrl+F5" :icon="firstIcon" :disabled="!doc?.frames.length" @click="frame.commands.execute('pixen.first-frame')" />
      <FrameIconButton label="Previous frame" shortcut="F5" icon="codicon:chevron-left" :disabled="!doc" @click="frame.commands.execute('pixen.previous-frame')" />
      <FrameIconButton
        :label="s.playing && s.direction === -1 ? 'Pause reverse playback' : 'Play in reverse'"
        :icon="s.playing && s.direction === -1 ? 'codicon:debug-pause' : 'codicon:play'"
        :flip-horizontal="!(s.playing && s.direction === -1)"
        shortcut="F6" :disabled="!doc || doc.frames.length < 2" @click="frame.commands.execute('pixen.play-reverse')"
      />
      <FrameIconButton
        :label="s.playing && s.direction === 1 ? 'Pause forward playback' : 'Play forward'"
        :icon="s.playing && s.direction === 1 ? 'codicon:debug-pause' : 'codicon:play'"
        shortcut="F7" :disabled="!doc || doc.frames.length < 2" @click="frame.commands.execute('pixen.play-forward')"
      />
      <FrameIconButton label="Next frame" shortcut="F8" icon="codicon:chevron-right" :disabled="!doc" @click="frame.commands.execute('pixen.next-frame')" />
      <FrameIconButton label="Last Frame" shortcut="Ctrl+F8" :icon="lastIcon" :disabled="!doc?.frames.length" @click="frame.commands.execute('pixen.last-frame')" />
    </div>
  </fieldset>
</template>
