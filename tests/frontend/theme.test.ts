import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { parseTheme } from '@frame/core';

test('Pixen YAML is a native Frame theme with the application accent and Pixen artwork', () => {
  const theme = parseTheme(readFileSync('assets/themes/pixen.yaml', 'utf8'), { format: 'yaml' });
  expect(theme.id).toBe('pixen');
  expect(theme.name).toBe('Pixen');
  expect(theme.appearance).toBe('dark');
  expect(theme.tokens['--frame-accent']).toBe('#FE725E');
  expect(readFileSync('assets/pixen.svg', 'utf8')).toContain('fill="#D25748"');
  expect(theme.tokens['--frame-font-size-body']).toBe('13px');
  expect(theme.tokens['--frame-button-frame-button-primary-color']).toBe('var(--frame-accent-foreground)');
  expect(theme.tokens['--frame-quick-selection']).toBe('var(--frame-accent)');
  expect(theme.tokens['--frame-command-result-active-color']).toBe('var(--frame-accent-foreground)');
});

test('Pixen views share Frame tokens without a second color registry', () => {
  const css = readFileSync('src/style.css', 'utf8');
  const app = readFileSync('src/App.vue', 'utf8');
  expect(css).not.toContain('--pixen-');
  expect(app).not.toContain('pixenTheme(');
  expect(app).toContain('parseTheme');
});
