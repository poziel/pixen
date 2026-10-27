import { expect, test } from 'bun:test';
import { createAiSettings } from '../../src/services/aiSettings';

test('AI preferences restore provider-specific models without connecting a provider', () => {
  let saved: string | null = null;
  const storage = { getItem: () => saved, setItem: (_key: string, value: string) => { saved = value; } };
  const settings = createAiSettings(storage);
  settings.state.models.codex = 'codex-artist';
  settings.state.provider = 'claude';
  settings.state.models.claude = 'claude-artist';
  settings.dispose();
  const restored = createAiSettings(storage);
  expect(restored.selection()).toEqual({provider:'claude',model:'claude-artist'});
  restored.state.provider = 'codex';
  expect(restored.selection()).toEqual({provider:'codex',model:'codex-artist'});
  restored.dispose();
});


test('legacy appearance overrides are ignored and only user preferences are saved', () => {
  let saved = JSON.stringify({ busyColor: '#cc8800', welcomeMessage: 'Old override' });
  const settings = createAiSettings({ getItem: () => saved, setItem: (_key, value) => { saved = value; } });
  expect('busyColor' in settings.state).toBe(false);
  expect('welcomeMessage' in settings.state).toBe(false);
  settings.state.provider = 'claude';
  expect(JSON.parse(saved)).toEqual({ provider: 'claude', models: {} });
  settings.dispose();
});

test('provider tests discover live models and discard an unavailable saved model', async () => {
  const calls: unknown[] = [];
  const settings = createAiSettings(undefined, (async (command: string, args: unknown) => {
    calls.push([command, args]);
    return [{ id: 'live-model', label: 'Live model' }];
  }) as typeof import('../../src/services/backend').call);
  settings.state.models.codex = 'obsolete';
  expect(settings.state.tests.codex).toBeUndefined();
  expect(calls).toEqual([]);
  const pending = settings.testProvider();
  expect(settings.state.tests.codex).toBe('testing');
  await pending;
  expect(settings.state.tests.codex).toBe('available');
  expect(settings.state.catalogs.codex).toEqual([{ id: 'live-model', label: 'Live model' }]);
  expect(settings.state.models.codex).toBe('');
  expect(calls).toEqual([['ai_test_provider', { provider: 'codex' }]]);
  settings.dispose();
});

test('failed retesting clears old models and marks the provider unavailable', async () => {
  const settings = createAiSettings(undefined, (async () => { throw new Error('Missing executable'); }) as typeof import('../../src/services/backend').call);
  settings.state.tests.codex = 'available';
  settings.state.catalogs.codex = [{ id: 'old', label: 'Old' }];
  await settings.testProvider();
  expect(String(settings.state.tests.codex)).toBe('unavailable');
  expect(settings.state.catalogs.codex).toEqual([]);
  settings.dispose();
});

test('switching providers during a test preserves independent results and prevents duplicate tests', async () => {
  let finish!: (models: {id:string;label:string}[]) => void;
  let calls = 0;
  const settings = createAiSettings(undefined, (() => { calls++; return new Promise<{id:string;label:string}[]>(resolve => { finish = resolve; }); }) as typeof import('../../src/services/backend').call);
  const pending = settings.testProvider();
  await settings.testProvider();
  settings.state.provider = 'claude';
  finish([{ id: 'codex-model', label: 'Codex model' }]);
  await pending;
  expect(calls).toBe(1);
  expect(settings.state.tests.codex).toBe('available');
  expect(settings.state.tests.claude).toBeUndefined();
  expect(settings.state.catalogs.claude).toBeUndefined();
  settings.dispose();
});
