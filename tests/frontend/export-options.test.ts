import { expect, test } from 'bun:test';
import { changeExportKind, needsBackground, createExportEntry, exportNames, formatOptions, validExport, validExportName } from '../../src/editor/exportOptions';

test('export kind resets incompatible formats and disables unsupported choices', () => {
  const entry = createExportEntry(1, 4, 0);
  changeExportKind(entry, 'video');
  expect(entry.format).toBe('mp4');
  expect(formatOptions('video').filter(o => !o.disabled).map(o => o.value)).toEqual(['mp4', 'webm']);
  changeExportKind(entry, 'animation'); expect(entry.format).toBe('gif');
  changeExportKind(entry, 'sheet'); expect(entry.format).toBe('gif');
  changeExportKind(entry, 'project'); expect(entry.format).toBe('pixen.json');
});
test('export names share a stem and never collide for repeated kinds and formats', () => {
  const entries = [createExportEntry(1, 4, 0), createExportEntry(2, 4, 0), createExportEntry(3, 4, 0), createExportEntry(4, 4, 0)];
  entries[1]!.mode = 'sheet'; entries[3]!.format = 'jpeg';
  expect(exportNames('hero', entries)).toEqual(['hero-frame.png', 'hero-sheet.png', 'hero-frame-2.png', 'hero.jpeg']);
});
test('export validates grid, frame, size, video and Windows filenames', () => {
  const entry = createExportEntry(1, 4, 0);
  expect(validExport(entry, 4, 32, 32)).toBe(true);
  entry.frame = 5; expect(validExport(entry, 4, 32, 32)).toBe(false);
  changeExportKind(entry, 'sheet'); entry.gridCount = 0; expect(validExport(entry, 4, 32, 32)).toBe(false);
  entry.gridCount = 2; entry.scale = 64; expect(validExport(entry, 4, 64, 64)).toBe(false);
  changeExportKind(entry, 'video'); expect(validExport(entry, 1, 1, 1)).toBe(false);
  for (const name of ['', '../hero', 'CON', 'a/b', 'a\\b', 'hero.', 'hero ', 'nul.txt']) expect(validExportName(name)).toBe(false);
  expect(validExportName('hero.v2')).toBe(true);
});

test('GIF export permits a transparent background', () => {
  expect(needsBackground('gif')).toBe(false);
  expect(needsBackground('png')).toBe(false);
  expect(needsBackground('webp')).toBe(false);
});
