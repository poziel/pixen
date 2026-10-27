<script setup lang="ts">
import { FrameIconButton, FrameDropdownMenu } from '@frame/core';
import { computed, markRaw, ref, onMounted, onBeforeUnmount, type Component } from 'vue';
import { SquareDashed, CircleDashed, Pentagon, WandSparkles, LassoSelect, Move, ZoomIn, Hand, Type, Square, Pencil, Eraser, PaintBucket, Pipette, Minus, Spline, RectangleHorizontal, Circle, Box, Paintbrush, Palette, Ellipsis } from '@lucide/vue';
import { useEditor } from '../editor';
import { tools, type Tool } from './raster';
import ToolTooltip from './ToolTooltip.vue';
const icons: Record<Tool, Component> = Object.fromEntries(Object.entries({
  rectangle: SquareDashed, ellipse: CircleDashed, polygon: Pentagon, color: Palette,
  lasso: LassoSelect, move: Move, zoom: ZoomIn, pan: Hand, text: Type,
  paper: Square, pencil: Pencil, eraser: Eraser, bucket: PaintBucket, eyedropper: Pipette,
  line:Minus,curve:Spline,'draw-rectangle':RectangleHorizontal,'draw-ellipse':Circle,isometric:Box,
  'paint-select':Paintbrush,wand:WandSparkles,
}).map(([id, component]) => [id, markRaw(component)])) as Record<Tool, Component>;
const s = useEditor().state;
const t = s.tools;
const activeLeft = computed(() => t.temporaryPicker ? 'eyedropper' : t.temporaryPan ? 'pan' : t.left);
const toolbar = ref<HTMLElement>();
const visibleCount = ref(tools.length);
const overflowOpen = ref(false);
const visibleTools = computed(() => tools.slice(0, visibleCount.value));
const overflowTools = computed(() => tools.slice(visibleCount.value).map(tool => ({
  id: tool.id, label: tool.label, icon: icons[tool.id],
  checked: activeLeft.value === tool.id || t.right === tool.id,
})));
const overflowLeft = computed(() => overflowTools.value.some(tool => tool.id === activeLeft.value));
const overflowRight = computed(() => overflowTools.value.some(tool => tool.id === t.right));
function assignmentBackground(left: boolean, right: boolean) {
  const blue = 'color-mix(in srgb, var(--drawing-left, #28a9f4) 80%, transparent)', orange = 'color-mix(in srgb, var(--drawing-right, #ff8b36) 80%, transparent)';
  return { background: left && right ? `linear-gradient(to right, ${blue} 50%, ${orange} 50%)` : left ? blue : right ? orange : undefined };
}
function toolBackground(id: string) { return assignmentBackground(activeLeft.value === id, t.right === id); }
let observer: ResizeObserver | undefined;
function measure() {
  const element = toolbar.value;
  if (!element?.parentElement) return;
  const gap = parseFloat(getComputedStyle(element.querySelector('.tool-grid')!).gap) || 4;
  const available = (element.closest('.frame-dock-lane')?.clientWidth ?? element.parentElement.clientWidth) - 4;
  const capacity = Math.max(1, Math.floor((available + gap) / (36 + gap)));
  visibleCount.value = capacity >= tools.length ? tools.length : Math.max(0, capacity - 1);
  if (visibleCount.value === tools.length) overflowOpen.value = false;
}
function selectOverflow(id: string, right = false) {
  if (!tools.some(tool => tool.id === id)) return;
  if (right) t.right = id as Tool;
  else t.left = id as Tool;
  overflowOpen.value = false;
  s.canvasFocus++;
}
onMounted(() => {
  observer = new ResizeObserver(measure);
  if (toolbar.value?.parentElement) observer.observe(toolbar.value.closest('.frame-dock-lane') ?? toolbar.value.parentElement);
  measure();
});
onBeforeUnmount(() => observer?.disconnect());
</script>
<template>
  <fieldset ref="toolbar" class="drawing-toolbar tool-control-group" :disabled="!s.snapshot?.document" :inert="!s.snapshot?.document" :aria-disabled="!s.snapshot?.document">
    <div class="tool-grid" role="toolbar" aria-label="Drawing tools">
      <ToolTooltip v-for="tool in visibleTools" :key="tool.id" :tool="tool.id" :label="tool.label" v-slot="{describedby,focus,hide}">
      <FrameIconButton :data-tool="tool.id" :size="36"
        :label="`${tool.label}${activeLeft===tool.id?' (left button)':''}${t.right===tool.id?' (right button)':''}`"
        :tooltip="false" :aria-describedby="describedby" @focus="focus" @blur="hide"
        :class="{'assigned-left':activeLeft===tool.id,'assigned-right':t.right===tool.id}"
        :aria-pressed="activeLeft===tool.id||t.right===tool.id"
        @click="t.left=tool.id;s.canvasFocus++;hide()" @contextmenu.prevent="t.right=tool.id;s.canvasFocus++;hide()">
        <component :is="icons[tool.id]" :size="20" :stroke-width="1.75" aria-hidden="true" />
        <small v-if="activeLeft===tool.id" class="tool-assignment-left" aria-hidden="true">L</small>
        <small v-if="t.right===tool.id" class="tool-assignment-right" aria-hidden="true">R</small>
      </FrameIconButton>
      </ToolTooltip>
      <FrameDropdownMenu v-if="overflowTools.length" v-model:open="overflowOpen" :items="overflowTools"
        class="drawing-tools-menu" label="More drawing tools" :content-props="{side:'top',align:'end'}"
        @select="item => selectOverflow(item.id)">
        <template #trigger><FrameIconButton label="More drawing tools" :size="36"
          :class="{'assigned-left':overflowLeft,'assigned-right':overflowRight}"
          :aria-pressed="overflowLeft || overflowRight">
          <Ellipsis :size="20" aria-hidden="true" />
          <small v-if="overflowLeft" class="tool-assignment-left" aria-hidden="true">L</small>
          <small v-if="overflowRight" class="tool-assignment-right" aria-hidden="true">R</small>
        </FrameIconButton></template>
        <template #item="{item}">
          <span class="overflow-tool-item" :data-overflow-tool="item.id" :style="toolBackground(item.id)" @click.stop="selectOverflow(item.id)" @contextmenu.prevent.stop="selectOverflow(item.id, true)">
            <span class="overflow-tool-icon"><component :is="icons[item.id as Tool]" :size="18" aria-hidden="true" /></span>
            <span>{{item.label}}</span>
            <small>{{activeLeft === item.id ? 'L' : ''}}{{t.right === item.id ? 'R' : ''}}</small>
          </span>
        </template>
      </FrameDropdownMenu>
    </div>
  </fieldset>
</template>

<style scoped>
.overflow-tool-item { display:flex;align-items:center;gap:8px;width:100%;grid-column:1 / -1;padding:6px 8px;border-radius:3px;box-sizing:border-box; }
.overflow-tool-icon { display:inline-flex;padding:2px;border-radius:3px; }
.overflow-tool-item small { margin-left:auto; }
</style>
