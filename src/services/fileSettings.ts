import type { SettingsStorage } from '@frame/core';
import { call } from './backend';
import { providers, type AiSettings } from './aiSettings';

const modelPrefix = 'pixen.ai.models.';
const transient = ['pixen.reference', 'pixen.follow', 'pixen.ai.model', 'pixen.representation'];
type Values = Record<string, string | number | boolean>;

/** Read old browser keys only when the native settings file does not exist. */
export function legacyPreferences(storage?: Pick<Storage, 'getItem'>): Values {
  const values: Values = {};
  try {
    const interval = Number(storage?.getItem('pixen.recoveryInterval'));
    if (Number.isInteger(interval) && interval >= 1 && interval <= 3600) values['pixen.recoveryInterval'] = interval;
  } catch { /* Keep other independently recoverable preferences. */ }
  try {
    const saved = JSON.parse(storage?.getItem('pixen.ai-settings.v1') ?? '{}');
    if (providers.some(provider => provider.id === saved.provider)) values['pixen.ai.provider'] = saved.provider;
    for (const provider of providers) if (typeof saved.models?.[provider.id] === 'string') values[modelPrefix + provider.id] = saved.models[provider.id];
  } catch { /* An invalid old AI preference does not block other settings. */ }
  try { const theme = storage?.getItem('pixen.theme'); if (theme) values['pixen.theme'] = theme; } catch { /* Use Pixen's built-in default. */ }
  return values;
}

/** Delay Frame hydration until custom themes and settings contributions exist. */
export function createFileSettings(
  options: {
    ai: AiSettings;
    legacy?: Pick<Storage, 'getItem'>;
    prepare(values: Values): void;
  },
  transport: typeof call = call,
) {
  let start!: () => void;
  const ready = new Promise<void>(resolve => { start = resolve; });
  const storage: SettingsStorage = {
    async read() {
      await ready;
      const json = await transport<string | null>('read_settings_file');
      const values: Values = json == null ? legacyPreferences(options.legacy) : JSON.parse(json);
      if (!values || typeof values !== 'object' || Array.isArray(values)) throw new Error('Settings must be a JSON object.');
      for (const [id, value] of Object.entries(values)) {
        if (!['string', 'boolean', 'number'].includes(typeof value) || typeof value === 'number' && !Number.isFinite(value)) throw new Error(`Invalid setting value: ${id}`);
      }
      // Recover the previously selected single-model key as well as provider-specific models.
      const provider = values['pixen.ai.provider'];
      if (providers.some(item => item.id === provider) && typeof values['pixen.ai.model'] === 'string' && values[modelPrefix + provider] === undefined) values[modelPrefix + provider] = values['pixen.ai.model'];
      for (const id of transient) delete values[id];
      options.prepare(values);
      return JSON.stringify(values);
    },
    async write(json) {
      const values: Values = JSON.parse(json);
      for (const id of transient) delete values[id];
      await transport('write_settings_file', { json: JSON.stringify(values, Object.keys(values).sort(), 2) + '\n' });
    },
  };
  return { storage, start };
}
