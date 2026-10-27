import { expect, test } from 'bun:test';
import { rememberColor } from '../../src/editor/tools/recent';

test('recent colors normalize equivalent values, promote reuse, and stay bounded', () => {
  const colors: string[] = [];
  rememberColor(colors, '#ff0000');
  rememberColor(colors, '#00ff0080');
  rememberColor(colors, '#FF0000FF');
  expect(colors).toEqual(['#ff0000ff', '#00ff0080']);
  for (let i=0;i<40;i++) rememberColor(colors, '#'+i.toString(16).padStart(6,'0'));
  expect(colors).toHaveLength(32);
  expect(colors[0]).toBe('#000027ff');
});
