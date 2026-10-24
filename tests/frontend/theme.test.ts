import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { parseTheme } from '@frame/core';

test('Pixloom YAML is a native Frame theme with the logo accent', () => {
  const theme = parseTheme(readFileSync('assets/themes/pixloom.yaml', 'utf8'), { format: 'yaml' });
  expect(theme.id).toBe('pixloom');
  expect(theme.name).toBe('Pixloom');
  expect(theme.appearance).toBe('dark');
  expect(theme.tokens['--frame-accent']).toBe('#FE725E');
  expect(readFileSync('assets/pixloom.svg', 'utf8')).toContain('fill="#FE725E"');
  expect(theme.tokens['--frame-font-size-body']).toBe('13px');
  expect(theme.tokens['--frame-button-frame-button-primary-color']).toBe('var(--frame-accent-foreground)');
  expect(theme.tokens['--frame-quick-selection']).toBe('var(--frame-accent)');
  expect(theme.tokens['--frame-command-result-active-color']).toBe('var(--frame-accent-foreground)');
});

test('Pixloom views share Frame tokens without a second color registry', () => {
  const css = readFileSync('src/style.css', 'utf8');
  const app = readFileSync('src/App.vue', 'utf8');
  expect(css).not.toContain('--pixloom-');
  expect(app).not.toContain('pixloomTheme(');
  expect(app).toContain('parseTheme');
  const registry = JSON.parse(readFileSync('assets/themes/manifest.json', 'utf8'));
  expect(registry.default).toBe('pixloom');
  expect(registry.themes.pixloom).toBe('pixloom.yaml');
});
