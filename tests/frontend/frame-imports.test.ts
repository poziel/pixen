import { expect, test } from 'bun:test';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import * as Frame from '@frame/core';

test('Pixen runtime imports exist in the installed Frame package', () => {
  const exports = new Set(Object.keys(Frame));
  const files = readdirSync('src', { recursive: true, withFileTypes: true });
  let checked = 0;
  for (const file of files) {
    if (!file.isFile() || !/\.(vue|ts)$/.test(file.name)) continue;
    const source = readFileSync(join(file.parentPath, file.name), 'utf8');
    for (const match of source.matchAll(/import\s*\{([^}]+)\}\s*from\s*['"]@frame\/core['"]/g)) {
      for (const item of match[1].split(',')) {
        const name = item.trim();
        if (!name || name.startsWith('type ')) continue;
        expect(exports.has(name.split(/\s+as\s+/)[0]), `${file.name} imports missing Frame export ${name}`).toBe(true);
        checked++;
      }
    }
  }
  expect(checked).toBeGreaterThan(0);
});
