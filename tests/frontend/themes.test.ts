import { expect, test } from 'bun:test';
import { createThemes, type ThemeConfig } from '../../src/services/themes';
import type { FrameTheme } from '@frame/core';
import type { call } from '../../src/services/backend';

const theme = (id: string, name = id) => JSON.stringify({ version: 1, id, name, appearance: 'dark', tokens: { 'frame.accent': '#123456' } });
function fixture(files: ThemeConfig['files'], saved: string | null = null, selected?: string) {
  let applied: FrameTheme | undefined;
  const storage = { getItem: () => saved, setItem: (_key: string, value: string) => { saved = value; } };
  const catalog = { files, selected };
  const service = createThemes({ theme: { set(value: FrameTheme) { applied = value; } } } as Parameters<typeof createThemes>[0], storage, (async () => catalog) as typeof call);
  return { service, applied: () => applied, saved: () => saved };
}
const file = (name: string, text: string) => ({ file_name: name, source: { text, format: 'json' as const } });

test('discovers valid themes and silently excludes invalid documents, empty themes and reserved IDs', async () => {
  const { service } = fixture([
    file('custom.json', theme('custom', 'Custom theme')),
    file('settings.json', '{"unrelated":true}'),
    file('broken.json', 'not json'),
    file('empty.json', '{}'),
    file('replacement.json', theme('pixen')),
    file('legacy.json', theme('PIXLOOM')),
    file('duplicate.json', theme('custom')),
  ]);
  await service.load();
  expect(service.state.options).toEqual([{value:'pixen',label:'Pixen (built-in)'},{value:'custom',label:'Custom theme'}]);
  expect(service.state.selected).toBe('pixen');
});

test('discovery does not auto-apply a custom theme and selection persists separately', async () => {
  const { service, applied, saved } = fixture([file('custom.json', theme('custom'))]);
  await service.load();
  expect(applied()?.id).toBe('pixen');
  service.select('custom');
  expect(applied()?.id).toBe('custom');
  expect(saved()).toBe('custom');
  service.select('pixen');
  expect(applied()?.tokens['--frame-accent']).toBe('#FE725E');
});

test('restores a discovered selection and falls back when that theme was removed', async () => {
  const found = fixture([file('custom.json', theme('custom'))], 'custom');
  await found.service.load();
  expect(found.applied()?.id).toBe('custom');
  const missing = fixture([], 'custom');
  await missing.service.load();
  expect(missing.applied()?.id).toBe('pixen');
});

test('environment selection accepts a discovered filename; token-only themes use their filename', async () => {
  const { service, applied } = fixture([file('custom.json', '{"frame.accent":"#123456"}')], null, 'custom.json');
  await service.load();
  expect(service.state.selected).toBe('custom');
  expect(applied()?.tokens['--frame-accent']).toBe('#123456');
});
