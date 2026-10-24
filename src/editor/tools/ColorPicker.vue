<script setup lang="ts">
import { computed, ref, watch, onMounted } from 'vue';
import { useEditor } from '../editor';
import { hexToHsv, hsvToHex, rgba } from './raster';
const t = useEditor().state.tools;
const surface = ref<HTMLCanvasElement>();
const color = computed(() => t[t.colorTarget]);
const hsv = ref<[
    number,
    number,
    number
]>(hexToHsv(color.value));
const alpha = ref(255);
let authoredColor: string | undefined;
watch(color, c => {
    if (c === authoredColor) return;
    hsv.value = hexToHsv(c); alpha.value = rgba(c)[3];
}, { immediate: true });
function update() {
    authoredColor = hsvToHex(...hsv.value) + (alpha.value === 255 ? '' : Math.round(alpha.value).toString(16).padStart(2, '0'));
    t[t.colorTarget] = authoredColor;
}
function draw() {
    const c = surface.value;
    if (!c)
        return;
    const ctx = c.getContext('2d')!, size = 220;
    const image = ctx.createImageData(size, size);
    for (let y = 0; y < size; y++)
        for (let x = 0; x < size; x++) {
            const dx = (x - 109.5) / 109.5, dy = (y - 109.5) / 109.5, r = Math.hypot(dx, dy);
            let value: string | null = null;
            if (t.picker === 'rectangle')
                value = hsvToHex(hsv.value[0], x / 219, 1 - y / 219);
            else if (t.picker === 'circle' && r <= 1)
                value = hsvToHex((Math.atan2(dy, dx) * 180 / Math.PI + 360) % 360, r, hsv.value[2]);
            else if (t.picker === 'wheel') {
                if (r >= .78 && r <= 1)
                    value = hsvToHex((Math.atan2(dy, dx) * 180 / Math.PI + 360) % 360, 1, 1);
                else if (Math.abs(dx) <= .52 && Math.abs(dy) <= .52)
                    value = hsvToHex(hsv.value[0], (dx + .52) / 1.04, 1 - (dy + .52) / 1.04);
            }
            if (value)
                image.data.set(rgba(value), (y * size + x) * 4);
        }
    ctx.putImageData(image, 0, 0);
    let x: number, y: number;
    if (t.picker === 'circle') {
        const a = hsv.value[0] * Math.PI / 180;
        x = 109.5 + Math.cos(a) * hsv.value[1] * 109.5;
        y = 109.5 + Math.sin(a) * hsv.value[1] * 109.5;
    }
    else if (t.picker === 'wheel') {
        x = (.48 + 1.04 * hsv.value[1]) * 109.5;
        y = (.48 + 1.04 * (1 - hsv.value[2])) * 109.5;
    }
    else {
        x = hsv.value[1] * 219;
        y = (1 - hsv.value[2]) * 219;
    }
    ctx.beginPath();
    ctx.arc(x, y, 4, 0, Math.PI * 2);
    ctx.strokeStyle = 'black';
    ctx.lineWidth = 3;
    ctx.stroke();
    ctx.strokeStyle = 'white';
    ctx.lineWidth = 1.5;
    ctx.stroke();
}
watch(() => [...hsv.value, t.picker], draw);
onMounted(draw);
let dragMode = '';
function pick(e: PointerEvent, start = false) {
    if (e.button !== 0 && start)
        return;
    const c = surface.value!, b = c.getBoundingClientRect(), x = (e.clientX - b.left) / b.width, y = (e.clientY - b.top) / b.height, dx = (x - .5) * 2, dy = (y - .5) * 2, r = Math.hypot(dx, dy);
    if (start) {
        c.setPointerCapture(e.pointerId);
        dragMode = t.picker === 'wheel' ? (r >= .78 ? 'hue' : 'square') : t.picker;
    }
    if (!c.hasPointerCapture(e.pointerId))
        return;
    if (dragMode === 'hue')
        hsv.value[0] = (Math.atan2(dy, dx) * 180 / Math.PI + 360) % 360;
    else if (dragMode === 'circle') {
        hsv.value[0] = (Math.atan2(dy, dx) * 180 / Math.PI + 360) % 360;
        hsv.value[1] = Math.min(1, r);
    }
    else {
        const inset = dragMode === 'square' ? .24 : 0, span = 1 - inset * 2;
        hsv.value[1] = Math.max(0, Math.min(1, (x - inset) / span));
        hsv.value[2] = 1 - Math.max(0, Math.min(1, (y - inset) / span));
    }
    update();
}
function hex(e: Event) { const value = (e.target as HTMLInputElement).value; if (/^#[\da-f]{6}([\da-f]{2})?$/i.test(value))
    t[t.colorTarget] = value; }
</script>
<template>
  <section class="color-picker" aria-label="Color picker">
    <div class="swatches">
      <button v-for="target in (['primary','secondary'] as const)" :key="target" :style="{background:t[target]}" :aria-label="`${target} color ${t[target]}`" :aria-pressed="t.colorTarget===target" @click="t.colorTarget=target"><span>{{ target==='primary'?'1':'2' }}</span></button>
      <button aria-label="Swap colors (X)" title="Swap colors (X)" @click="[t.primary,t.secondary]=[t.secondary,t.primary]">⇄</button>
      <input :value="color" aria-label="Hex color, optional alpha" maxlength="9" @change="hex" />
    </div>
    <label>Color picker <select v-model="t.picker" aria-label="Color picker mode"><option value="rectangle">HSV Rectangle</option><option value="wheel">HSB Wheel</option><option value="circle">HSV Circle</option></select></label>
    <canvas ref="surface" width="220" height="220" class="color-surface" aria-label="Color surface; use HSV controls below for keyboard input" @pointerdown="pick($event,true)" @pointermove="pick($event)" @pointerup="surface?.releasePointerCapture($event.pointerId)" @pointercancel="dragMode=''" />
    <label v-if="t.picker!=='circle'">Hue <input v-model.number="hsv[0]" type="range" min="0" max="359" class="hue-slider" @input="update" /></label>
    <label v-else>Value <input v-model.number="hsv[2]" type="range" min="0" max="1" step="0.01" @input="update" /></label>
    <div class="hsv-fields"><label>H<input v-model.number="hsv[0]" type="number" min="0" max="359" @change="hsv[0]=Math.max(0,Math.min(359,hsv[0]));update()" /></label><label>S %<input :value="Math.round(hsv[1]*100)" type="number" min="0" max="100" @change="hsv[1]=Math.max(0,Math.min(100,Number(($event.target as HTMLInputElement).value)))/100;update()" /></label><label>V %<input :value="Math.round(hsv[2]*100)" type="number" min="0" max="100" @change="hsv[2]=Math.max(0,Math.min(100,Number(($event.target as HTMLInputElement).value)))/100;update()" /></label></div>
    <label>Alpha <input v-model.number="alpha" type="range" min="0" max="255" @input="update" /><output>{{ alpha }}</output></label>
  </section>
</template>

