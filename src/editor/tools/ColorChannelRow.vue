<script setup lang="ts">
import { FrameTooltip, FrameColorSlider, FrameSlider, FrameNumberField, type FrameColor } from '@frame/core';
const props = withDefaults(defineProps<{
  label: string; name: string; modelValue: number; min: number; max: number; step: number;
  color?: FrameColor; channel?: 'red'|'green'|'blue'|'hue'|'saturation'|'brightness'|'alpha';
  colorSpace?: 'rgb'|'hsb'; factor?: number; gradient?: string;
}>(), { factor:1 });
const emit = defineEmits<{ 'update:modelValue': [value: number] }>();
function update(value: number) { if (Number.isFinite(value)) emit('update:modelValue',Math.max(props.min,Math.min(props.max,value))); }
function numberInput(event: Event) {
  const text = (event.target as HTMLInputElement).value.trim().replace(/,/g,'');
  // Keep incomplete decimal/negative entries editable while previewing valid numbers immediately.
  if (!text || text==='-' || text.endsWith('.')) return;
  update(Number(text));
}
function colorUpdate(value: FrameColor) {
  if (props.channel==='alpha') return update(value.alpha*props.factor);
  // Reka represents hue edits in HSL even when the surrounding picker uses HSV.
  if (props.channel==='hue' && 'h' in value) return update(value.h*props.factor);
  if (value.space==='rgb') {
    const index = props.channel==='red'?'r':props.channel==='green'?'g':'b';
    update(value[index]*props.factor);
  } else if (value.space==='hsb') {
    const index = props.channel==='hue'?'h':props.channel==='saturation'?'s':'b';
    update(value[index]*props.factor);
  }
}
</script>
<template>
  <div class="color-channel-row" :data-channel="label" :style="gradient?{'--channel-gradient':gradient}:undefined" :class="{'custom-gradient':!!gradient}">
    <FrameTooltip :text="name" v-slot="{describedby}"><span class="color-channel-label" :aria-describedby="describedby">{{label}}</span></FrameTooltip>
    <FrameColorSlider v-if="channel&&color" :model-value="color" :channel="channel" thumb-alignment="overflow" :color-space="colorSpace" :step="step/factor" :aria-label="name+' channel'" @update:color="colorUpdate" />
    <FrameSlider v-else :model-value="modelValue" :min="min" :max="max" :step="step" :aria-label="name+' channel'" @update:model-value="update" />
    <FrameNumberField size="compact" controls="stacked" :model-value="modelValue" :min="min" :max="max" :step="step" :label="name+' value'" @input="numberInput" @update:model-value="update" />
  </div>
</template>
