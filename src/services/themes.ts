import { reactive } from 'vue';
import { parseTheme, type Frame, type FrameTheme, type FrameThemeFormat } from '@frame/core';
import { call } from './backend';
import pixenThemeText from '../../assets/themes/pixen.yaml?raw';

export interface ThemeSource { text: string; format: FrameThemeFormat }
export interface ThemeFile { file_name: string; source: ThemeSource }
export interface ThemeConfig { builtin: ThemeSource; files: ThemeFile[]; selected?: string | null }
const builtin = parseTheme(pixenThemeText, { format: 'yaml' });

/** Frame validates discovered files; custom themes cannot replace the built-in ID. */
export function createThemes(frame: Pick<Frame, 'theme'>, storage?: Pick<Storage, 'getItem' | 'setItem'>, transport: typeof call = call) {
  const themes = new Map<string, FrameTheme>([[builtin.id, builtin]]);
  const state = reactive({ selected: builtin.id, options: [{ value: builtin.id, label: 'Pixen (built-in)' }] });
  function select(id: string, persist = true) {
    const theme = themes.get(id);
    if (!theme) return;
    frame.theme.set(theme);
    state.selected = id;
    if (persist) { try { storage?.setItem('pixen.theme', id); } catch { /* The current session still uses the selected theme. */ } }
  }
  async function load() {
    let catalog: ThemeConfig;
    try { catalog = await transport<ThemeConfig>('theme_config'); } catch { return; }
    if (!Array.isArray(catalog?.files)) return;
    const fileIds = new Map<string, string>();
    for (const file of catalog.files) {
      try {
        const stem = file.file_name.replace(/\.[^.]+$/, '');
        const theme = parseTheme(file.source.text, { format: file.source.format, id: stem, name: stem });
        if (!Object.keys(theme.tokens).length || ['pixen', 'pixloom'].includes(theme.id.toLowerCase()) || themes.has(theme.id)) continue;
        themes.set(theme.id, theme);
        fileIds.set(file.file_name, theme.id);
        state.options.push({ value: theme.id, label: theme.name });
      } catch { /* Unsupported documents and invalid themes are silently ignored. */ }
    }
    let saved: string | null = null;
    try { saved = storage?.getItem('pixen.theme') ?? null; } catch { /* Use the built-in default. */ }
    const requested = catalog.selected ?? saved ?? builtin.id;
    const id = fileIds.get(requested) ?? requested;
    select(themes.has(id) ? id : builtin.id, false);
  }
  return { state, select, load };
}
export type ThemeSettings = ReturnType<typeof createThemes>;
