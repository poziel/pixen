import { reactive, watch, type InjectionKey, inject } from 'vue';
import type { AgentProvider } from '@frame/core/modules/chat';
import { call } from './backend';

export const providers: { id: AgentProvider; label: string }[] = [
  { id: 'codex', label: 'Codex' }, { id: 'claude', label: 'Claude' },
  { id: 'gemini', label: 'Gemini' }, { id: 'copilot', label: 'GitHub Copilot' },
  { id: 'opencode', label: 'OpenCode' },
];
export const defaultAiWelcome = 'Describe what you want to create or change. Each message includes a fresh artwork preview. Attach images or text files for reference.';
export interface ModelOption { id: string; label: string }
export function createAiSettings(storage?: Pick<Storage, 'getItem' | 'setItem'>, transport: typeof call = call) {
  const key = 'pixen.ai-settings.v1';
  const state = reactive({ provider: 'codex' as AgentProvider, models: {} as Partial<Record<AgentProvider, string>>, catalogs: {} as Partial<Record<AgentProvider, ModelOption[]>>, loading: false, error: '', tests: {} as Partial<Record<AgentProvider, 'testing' | 'available' | 'unavailable'>> });
  try {
    const saved = JSON.parse(storage?.getItem(key) ?? '{}');
    if (providers.some(p => p.id === saved.provider)) state.provider = saved.provider;
    for (const p of providers) if (typeof saved.models?.[p.id] === 'string') state.models[p.id] = saved.models[p.id];
  } catch { /* A damaged preference does not prevent opening chat. */ }
  const stop = watch(() => [state.provider, state.models], () => {
    try { storage?.setItem(key, JSON.stringify({ provider: state.provider, models: state.models })); }
    catch { state.error = 'AI preferences could not be saved.'; }
  }, { deep: true, flush: 'sync' });
  let requests = 0;
  async function refresh(provider = state.provider) {
    requests++; state.loading = true; state.error = '';
    try { state.catalogs[provider] = await call<ModelOption[]>('ai_models', { provider }); }
    catch (e) { state.error = String(e); }
    finally { if (--requests === 0) state.loading = false; }
  }
  async function testProvider(provider = state.provider) {
    if (state.tests[provider] === 'testing') return;
    state.tests[provider] = 'testing';
    state.catalogs[provider] = [];
    try {
      const models = await transport<ModelOption[]>('ai_test_provider', { provider });
      state.catalogs[provider] = models;
      if (state.models[provider] && !models.some(model => model.id === state.models[provider])) state.models[provider] = '';
      state.tests[provider] = 'available';
    } catch {
      state.tests[provider] = 'unavailable';
    }
  }
  function selection() { return { provider: state.provider, model: state.models[state.provider] ?? '' }; }
  return { state, selection, refresh, testProvider, dispose: stop };
}
export type AiSettings = ReturnType<typeof createAiSettings>;
export const AiSettingsKey: InjectionKey<AiSettings> = Symbol('AI settings');
export function useAiSettings() { const value = inject(AiSettingsKey); if (!value) throw new Error('AI settings are missing'); return value; }
