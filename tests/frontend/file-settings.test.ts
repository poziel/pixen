import { expect, test } from 'bun:test';
import { createFrame } from '@frame/core';
import { reactive } from 'vue';
import { createAiSettings, providers } from '../../src/services/aiSettings';
import { createFileSettings, legacyPreferences } from '../../src/services/fileSettings';
import type { call } from '../../src/services/backend';

function fixture(initial: string | null, legacyValues: Record<string, string> = {}, failWrites = false) {
  let disk = initial;
  const writes: string[] = [];
  const legacy = { getItem: (key: string) => legacyValues[key] ?? null };
  const ai = createAiSettings();
  const state = reactive({ recovery: 30, background: 'checkerboard' });
  const file = createFileSettings({ ai, legacy, prepare() {} }, (async (command: string, args?: {json:string}) => {
    if (command === 'read_settings_file') return disk;
    if (command === 'write_settings_file') {
      if (failWrites) throw new Error('disk full');
      disk = args!.json; writes.push(disk);
    }
  }) as typeof call);
  const frame = createFrame({ settings: { storage: file.storage } });
  frame.settings.register({ id: 'test', title: 'Settings', settings: [
    { id: 'pixen.recoveryInterval', label: 'Recovery', type: 'number', default: 30, min: 1, max: 3600, getValue: () => state.recovery, onChange: value => { state.recovery = Number(value); } },
    { id: 'pixen.background', label: 'Background', type: 'select', default: 'checkerboard', options: [{value:'checkerboard',label:'Checkerboard'},{value:'black',label:'Black'}], getValue: () => state.background, onChange: value => { state.background = String(value); } },
    { id: 'pixen.ai.provider', label: 'Provider', type: 'select', default: 'codex', options: providers.map(p => ({value:p.id,label:p.label})), getValue: () => ai.state.provider, onChange: value => { ai.state.provider = value as typeof ai.state.provider; } },
    { id: 'pixen.ai.model', persist: false, label: 'Model', type: 'text', default: '', getValue: () => ai.state.models[ai.state.provider] ?? '', onChange: value => { ai.state.models[ai.state.provider] = String(value); } },
    ...providers.map(provider => ({ id: `pixen.ai.models.${provider.id}`, label: 'Model', type: 'text' as const, default: '', visible: false, getValue: () => ai.state.models[provider.id] ?? '', onChange: (value: string | number | boolean) => { ai.state.models[provider.id] = String(value); } })),
  ] });
  return { file, frame, state, ai, writes, disk: () => disk, legacyValues, dispose() { frame.dispose(); ai.dispose(); } };
}

test('Frame migrates legacy recovery and all provider models, then saves edits and restores from disk', async () => {
  const old = { 'pixen.recoveryInterval': '47', 'pixen.ai-settings.v1': JSON.stringify({provider:'claude',models:{codex:'codex-saved',claude:'claude-saved'}}) };
  const first = fixture(null, old);
  expect(first.writes).toHaveLength(0);
  first.file.start(); await first.frame.settings.load(); await first.frame.settings.flush();
  expect(first.state.recovery).toBe(47);
  expect(first.ai.selection()).toEqual({provider:'claude',model:'claude-saved'});
  expect(first.ai.state.tests.claude).toBeUndefined();
  first.frame.settings.set('pixen.background', 'black');
  first.frame.settings.set('pixen.ai.provider', 'codex');
  first.frame.settings.set('pixen.ai.model', 'codex-new');
  await first.frame.settings.flush();
  const saved = JSON.parse(first.disk()!);
  expect(saved['pixen.ai.models.codex']).toBe('codex-new');
  expect(saved['pixen.ai.models.claude']).toBe('claude-saved');
  expect(saved['pixen.ai.model']).toBeUndefined();
  expect(first.legacyValues).toEqual(old);
  const second = fixture(first.disk(), { 'pixen.recoveryInterval': '999' });
  second.file.start(); await second.frame.settings.load(); await second.frame.settings.flush();
  expect(second.state.recovery).toBe(47);
  expect(second.state.background).toBe('black');
  expect(second.ai.selection()).toEqual({provider:'codex',model:'codex-new'});
  expect(second.ai.state.models.claude).toBe('claude-saved');
  first.dispose(); second.dispose();
});

test('existing empty configuration is authoritative and resetting removes the override', async () => {
  const app = fixture('{}', { 'pixen.recoveryInterval':'90' });
  app.file.start(); await app.frame.settings.load();
  expect(app.state.recovery).toBe(30);
  app.frame.settings.set('pixen.recoveryInterval', 60); await app.frame.settings.flush();
  expect(JSON.parse(app.disk()!)['pixen.recoveryInterval']).toBe(60);
  app.frame.settings.reset('pixen.recoveryInterval'); await app.frame.settings.flush();
  expect(JSON.parse(app.disk()!)['pixen.recoveryInterval']).toBeUndefined();
  app.dispose();
});

for (const invalid of ['{broken', '{"pixen.recoveryInterval":"bad"}', '{"unknown":null}']) {
  test(`invalid settings are reported and never overwritten: ${invalid}`, async () => {
    const app = fixture(invalid);
    app.file.start();
    await expect(app.frame.settings.load()).rejects.toThrow();
    expect(app.writes).toHaveLength(0);
    expect(app.disk()).toBe(invalid);
    app.dispose();
  });
}

test('failed migration writes retain legacy preferences for a later retry', async () => {
  const app = fixture(null, { 'pixen.recoveryInterval':'50' }, true);
  app.file.start(); await app.frame.settings.load();
  await expect(app.frame.settings.flush()).rejects.toThrow('disk full');
  expect(app.legacyValues['pixen.recoveryInterval']).toBe('50');
  expect(app.disk()).toBeNull();
  app.dispose();
});

test('legacy parsing salvages valid independent values and ignores obsolete appearance overrides', () => {
  const values = { 'pixen.recoveryInterval':'1.5', 'pixen.ai-settings.v1':'not json', 'pixen.theme':'custom' };
  expect(legacyPreferences({getItem:key => values[key as keyof typeof values] ?? null})).toEqual({'pixen.theme':'custom'});
});

test('existing background and speed preferences survive while the retired global color mode is removed', async () => {
  const app = fixture('{"pixen.background":"black","pixen.fps":"12","pixen.representation":"grayscale"}');
  app.file.start(); await app.frame.settings.load(); await app.frame.settings.flush();
  const saved = JSON.parse(app.disk()!);
  expect(saved['pixen.background']).toBe('black');
  expect(saved['pixen.fps']).toBe('12');
  expect(saved['pixen.representation']).toBeUndefined();
  app.dispose();
});

test('provider models migrate into the same JSON document that edits and resets them', async () => {
  const app = fixture('{"pixen.ai.provider":"codex","pixen.ai.model":"legacy-model"}');
  app.file.start(); await app.frame.settings.load(); await app.frame.settings.flush();
  expect(app.ai.selection().model).toBe('legacy-model');
  expect(JSON.parse(app.disk()!)['pixen.ai.model']).toBeUndefined();
  app.frame.settings.applyJSON('{"pixen.ai.models.codex":"json-model","pixen.ai.models.claude":"other-model"}');
  await app.frame.settings.flush();
  expect(app.ai.selection().model).toBe('json-model');
  expect(JSON.parse(app.disk()!)).toEqual(JSON.parse(app.frame.settings.toJSON()));
  app.frame.settings.applyJSON('{}'); await app.frame.settings.flush();
  expect(app.ai.state.models.codex).toBe(''); expect(app.ai.state.models.claude).toBe('');
  expect(JSON.parse(app.disk()!)).toEqual({}); app.dispose();
});
