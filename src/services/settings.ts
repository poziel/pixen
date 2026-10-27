import { markRaw } from 'vue';
import type { Frame, SettingDefinition } from '@frame/core';
import type { Editor } from '../editor/editor';
import type { AiSettings } from './aiSettings';
import { providers } from './aiSettings';
import type { ThemeSettings } from './themes';
import AiProviderField from '../views/AiProviderField.vue';
import AiModelField from '../views/AiModelField.vue';
import InstallationSettings from '../views/InstallationSettings.vue';
import type { StorageModeService } from './storageMode';

/** Application settings are registered once; Frame owns their presentation and actions. */
export function registerPixenSettings(frame: Frame, s: Editor['state'], ai: AiSettings, themes: ThemeSettings, storage: StorageModeService) {
  const options = (values: string[]) => values.map(value => ({ value, label: value[0].toUpperCase() + value.slice(1) }));
  const canvas: SettingDefinition[] = [
    { id: 'pixen.background', label: 'Default background', description: 'Use this backdrop for newly created or opened artworks. Existing artworks keep their current background.', type: 'select', default: 'checkerboard', options: options(['checkerboard', 'transparent', 'black', 'white']), getValue: () => s.defaults.background, onChange: value => { s.defaults.background = String(value); } },
    { id: 'pixen.reference', persist: false, label: 'Comparison reference', description: 'Choose a fixed frame to compare against, or follow neighboring frames.', type: 'select', default: '', get disabled() { return s.mode === 'composite'; }, get options() { return [{ value: '', label: 'Neighboring frames' }, ...(s.snapshot?.document?.frames ?? []).map((item, index) => ({ value: item.id, label: 'Frame ' + (index + 1) }))]; }, getValue: () => s.reference ?? '', onChange: value => { s.reference = String(value) || null; } },
    { id: 'pixen.follow', persist: false, label: 'Follow MCP activity', description: 'Follow the artwork and frame selected by MCP while viewing a managed document.', type: 'checkbox', default: true, get disabled() { return !s.snapshot?.managed; }, getValue: () => s.follow, onChange: value => { s.follow = Boolean(value); } },
  ];
  const unregister = [
    frame.settings.register({ id: 'pixen.installation', title: 'Installation', description: 'Choose where Pixen stores its data.', component: markRaw(InstallationSettings), props: { storage } }),
    frame.settings.register({ id: 'pixen.appearance', title: 'Appearance', description: 'Pixen is built in. Add compatible custom themes to the themes folder beside the executable and restart to discover them.', settings: [
      { id: 'pixen.theme', label: 'Theme', type: 'select', default: 'pixen', get options() { return themes.state.options; }, getValue: () => themes.state.selected, onChange: value => themes.select(String(value)) },
    ] }),
    frame.settings.register({ id: 'pixen.canvas', title: 'Canvas', description: 'Choose the starting background for new artworks. Use the View menu for the current artwork.', settings: canvas }),
    frame.settings.register({ id: 'pixen.playback', title: 'Playback', description: 'Choose the starting preview speed for new artworks. Existing previews and saved frame timings stay unchanged.', settings: [
      { id: 'pixen.fps', label: 'Default preview speed', description: 'Start newly created or opened artworks with frame timings or this FPS override.', type: 'select', default: '0', get options() { return [{value:'0',label:'Frame timings'},...[...new Set([0.5,1,3,5,8,15,20,...(s.defaults.fps>0?[s.defaults.fps]:[])])].sort((a,b)=>a-b).map(value=>({value:String(value),label:value+' FPS'}))]; }, getValue: () => String(s.defaults.fps), onChange: value => { s.defaults.fps = Number(value); } },
    ] }),
    frame.settings.register({ id: 'pixen.recovery', title: 'Recovery', description: 'Save changed artwork in the background without changing project files or retaining undo history.', settings: [
      { id: 'pixen.recoveryInterval', label: 'Save recovery file every (seconds)', description: 'Set the delay between background recovery snapshots of changed artwork.', type: 'number', default: 30, min: 1, max: 3600, step: 1, getValue: () => s.recoveryInterval, onChange: value => { if (!Number.isInteger(value)) throw new Error('Enter a whole number of seconds.'); s.recoveryInterval = Number(value); } },
    ] }),
    frame.settings.register({ id: 'pixen.ai', title: 'AI', description: 'Choose the provider and model for new art-assistant conversations. Existing conversations keep their current choices.', settings: [
      { id: 'pixen.ai.provider', label: 'Provider', description: 'Choose an AI provider, then test its connection to discover available models.', type: 'select', default: 'codex', options: providers.map(p => ({ value: p.id, label: p.label })), component: markRaw(AiProviderField), props: { ai }, getValue: () => ai.state.provider, onChange: value => { ai.state.provider = value as typeof ai.state.provider; } },
      { id: 'pixen.ai.model', persist: false, label: 'Model', description: 'Choose a discovered model or use the provider default. Test the provider first to enable this choice.', type: 'text', default: '', component: markRaw(AiModelField), props: { ai }, getValue: () => ai.state.models[ai.state.provider] ?? '', onChange: value => { ai.state.models[ai.state.provider] = String(value); } },
      ...providers.map(provider => ({ id: `pixen.ai.models.${provider.id}`, label: `${provider.label} model`, type: 'text' as const, default: '', visible: false, getValue: () => ai.state.models[provider.id] ?? '', onChange: (value: string | number | boolean) => { ai.state.models[provider.id] = String(value); } })),
    ] }),
  ];
  return () => unregister.forEach(remove => remove());
}
