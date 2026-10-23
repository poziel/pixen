<script setup lang="ts">
import { computed } from "vue";
import { FrameButton } from "@frame/core";
import { useEditor } from "./editor";
import Thumbnail from "./Thumbnail.vue";
const editor = useEditor(),
  s = editor.state;
const doc = computed(() => s.snapshot?.document);
</script>
<template>
  <div class="sidebar-content">
    <fieldset
      :disabled="editor.following()"
      style="border: 0; margin: 0; padding: 0"
    >
      <div class="playback-tools">
        <FrameButton
          label="Previous frame"
          icon="codicon:chevron-left"
          variant="ghost"
          :disabled="!doc"
          @click="editor.step(-1)"
        />
        <FrameButton
          :label="s.playing ? 'Pause' : 'Play'"
          :icon="s.playing ? 'codicon:debug-pause' : 'codicon:play'"
          :disabled="!doc || doc.frames.length < 2"
          @click="
            s.playing = !s.playing;
            s.sheet = false;
            s.mode = 'composite';
          "
        />
        <FrameButton
          label="Stop"
          icon="codicon:debug-stop"
          variant="ghost"
          :disabled="!doc"
          @click="editor.select(0)"
        />
        <FrameButton
          label="Next frame"
          icon="codicon:chevron-right"
          variant="ghost"
          :disabled="!doc"
          @click="editor.step(1)"
        />
      </div>
      <label class="field"
        >Preview speed
        <select v-model.number="s.speed">
          <option
            v-for="speed in [0.125, 0.25, 0.5, 1, 2, 4, 8]"
            :key="speed"
            :value="speed"
          >
            {{ speed }}×
          </option>
        </select></label
      >
      <p class="hint">Saved frame timing stays unchanged.</p>
      <p v-if="!doc" class="empty-note">
        Open animated artwork to see its timeline.
      </p>
      <button
        v-for="(frame, index) in doc?.frames"
        :key="frame.id"
        class="timeline-row"
        :class="{ selected: index === s.frame }"
        :aria-pressed="index === s.frame"
        @click="editor.select(index)"
      >
        <Thumbnail :frame-id="frame.id" /><span
          ><strong>Frame {{ index + 1 }}</strong
          ><small>{{ frame.duration_ms }} ms</small></span
        >
      </button>
    </fieldset>
  </div>
</template>
