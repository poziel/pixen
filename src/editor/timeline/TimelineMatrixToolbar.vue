<script setup lang="ts">
import { computed } from 'vue';
import { FrameButton, FrameDropdownMenu, FrameIconButton, useFrame, type FrameMenuItem } from '@frame/core';
import TimelineControls from './TimelineControls.vue';
import {useEditor} from '../editor';
import {previewSpeedItems,previewFps} from './previewSpeed';
import {Scissors} from '@lucide/vue';
export interface MatrixAction { id: string; label: string; icon: string; shortcut?:string; disabled: boolean; run: () => void }
const props = defineProps<{ creation: MatrixAction[]; cells: MatrixAction[] }>();
const frame = useFrame();
const editor=useEditor(),s=editor.state;
const speed=computed(()=>previewSpeedItems(s.fps).map(item=>({...item,disabled:editor.following()||!s.snapshot?.document})));
const maximize = computed(() => frame.layout.maximizedRegion === 'bottom');
const menu = (actions: MatrixAction[]): FrameMenuItem[] => actions.map(({ id, label, disabled,shortcut }) => ({ id, label, disabled,shortcut }));
const playback = computed<FrameMenuItem[]>(() => [
  ['first-frame', 'First frame'], ['previous-frame', 'Previous frame'], ['play-reverse', 'Play in reverse'],
  ['play-forward', 'Play forward'], ['next-frame', 'Next frame'], ['last-frame', 'Last frame'],
].map<FrameMenuItem>(([id, label]) => ({ id: `pixen.${id}`, label: label!, disabled: !frame.commands.isEnabled(`pixen.${id}`) })));
const all = computed<FrameMenuItem[]>(() => [
  { id: 'playback', label: 'Playback', children: playback.value },
  { id: 'create', label: 'Create', children: menu(props.creation) },
  { id: 'cells', label: 'Cels', children: menu(props.cells) },
]);
function select(item: FrameMenuItem) {
  if (item.type === 'separator' || item.disabled) return;
  if(item.id==='0'||previewFps.some(fps=>String(fps)===item.id)){if(!editor.following()){s.fps=Number(item.id);s.elapsed=0;}return;}
  const action = [...props.creation, ...props.cells].find(action => action.id === item.id);
  if (action) action.run(); else void frame.commands.execute(item.id);
}
</script>
<template>
  <div class="matrix-toolbar-container">
    <div class="matrix-tools" role="toolbar" aria-label="Timeline actions">
      <div class="playback-full"><TimelineControls /></div>
      <div class="playback-menu"><FrameDropdownMenu label="Timeline playback" :items="playback" @select="select"><template #trigger><FrameButton label="Playback" icon="play" /></template></FrameDropdownMenu></div>
      <div class="creation-full"><FrameButton v-for="action in creation" :key="action.id" :label="action.label" :icon="action.icon" :disabled="action.disabled" @click="action.run" /></div>
      <div class="creation-menu"><FrameDropdownMenu label="Create artwork structure" :items="menu(creation)" @select="select"><template #trigger><FrameButton label="Create" icon="add" /></template></FrameDropdownMenu></div>
      <div class="cells-full"><FrameIconButton v-for="action in cells" :key="action.id" :label="action.label" :icon="action.id==='cut'?Scissors:action.icon" :shortcut="action.shortcut" :disabled="action.disabled" @click="action.run" /></div>
      <div class="cells-menu"><FrameDropdownMenu label="Selected cels" :items="menu(cells)" @select="select"><template #trigger><FrameButton label="Cels" icon="list-selection" /></template></FrameDropdownMenu></div>
      <div class="all-menu"><FrameDropdownMenu label="Timeline actions menu" :items="all" @select="select"><template #trigger><FrameButton label="Actions" icon="ellipsis" /></template></FrameDropdownMenu></div>
      <span class="toolbar-space" />
      <div class="matrix-timing"><FrameDropdownMenu label="Preview speed" :items="speed" @select="select"><template #trigger><FrameButton aria-label="Preview speed" icon="watch" :tooltip="`Preview speed: ${s.fps>0?`${s.fps} FPS`:'Frame timings'}; saved durations are unchanged`" :disabled="editor.following()||!s.snapshot?.document"><span class="speed-label">{{s.fps>0?`${s.fps} FPS`:'Frame timings'}}</span></FrameButton></template></FrameDropdownMenu></div>
      <FrameIconButton :label="maximize ? 'Restore panel' : 'Maximize panel'" :icon="maximize ? 'screen-normal' : 'screen-full'" :aria-pressed="maximize" @click="frame.commands.execute('frame.panel.maximize')" />
      <FrameIconButton label="Hide panel" icon="close" @click="frame.commands.execute('frame.panel.close')" />
    </div>
  </div>
</template>
<style scoped>
.matrix-toolbar-container { container-type:inline-size; flex-shrink:0; }
.matrix-tools { display:flex; align-items:center; gap:4px; padding:4px 8px; border-bottom:1px solid var(--frame-line); min-width:0;white-space:nowrap; }
.matrix-timing {border-left:1px solid var(--frame-line);padding-left:4px;flex-shrink:0;}
.playback-full {display:flex;align-items:center;}
.creation-full,.cells-full { display:flex; align-items:center; gap:2px; border-left:1px solid var(--frame-line); padding-left:5px; }
.toolbar-space { flex:1; min-width:0; }
:deep(.playback-menu),:deep(.creation-menu),:deep(.cells-menu),:deep(.all-menu) { display:none; }
@container (max-width: 1000px) { .creation-full { display:none; } :deep(.creation-menu) { display:inline-flex; } }
@container (max-width: 700px) { .cells-full { display:none; } :deep(.cells-menu) { display:inline-flex; } }
@container (max-width: 600px) { .playback-full { display:none; } :deep(.playback-menu) { display:inline-flex; } }
@container (max-width: 600px) {.speed-label{display:none;}}
@container (max-width: 320px) { :deep(.creation-menu),:deep(.cells-menu),:deep(.playback-menu) { display:none; } :deep(.all-menu) { display:inline-flex; } }
</style>
