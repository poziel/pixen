import { expect, test } from 'bun:test';
import { cellKey, rectangleCells, selectCell, dragCells, groupCells } from '../../src/editor/timeline/matrixSelection';

const rows = ['highlights', 'face', 'body', 'background'];
const frames = Array.from({ length: 10 }, (_, i) => `frame-${i + 1}`);
const cell = (row: number, column: number) => ({ layerId: rows[row]!, frameId: frames[column]! });

test('Shift selects horizontal, vertical, and diagonal rectangles in either direction', () => {
  expect(rectangleCells(cell(0, 2), cell(0, 4), rows, frames)).toHaveLength(3);
  expect(rectangleCells(cell(0, 2), cell(3, 2), rows, frames)).toHaveLength(4);
  const diagonal = rectangleCells(cell(3, 4), cell(1, 2), rows, frames);
  expect(diagonal).toHaveLength(9);
  expect(diagonal).toContainEqual(cell(2, 3));
  expect(rectangleCells(cell(0, 0), { layerId: 'removed', frameId: 'frame-1' }, rows, frames)).toEqual([]);
});

test('Ctrl/Cmd toggles sparse cells across layers without selecting intervening artwork', () => {
  let state = { selected: [cell(1, 1)], anchor: cell(1, 1) };
  for (const target of [cell(1, 6), cell(3, 8), cell(2, 9)]) state = selectCell(state.selected, state.anchor, target, rows, frames, { additive: true });
  expect(state.selected).toEqual([cell(1, 1), cell(1, 6), cell(3, 8), cell(2, 9)]);
  state = selectCell(state.selected, state.anchor, cell(3, 8), rows, frames, { additive: true });
  expect(state.selected).not.toContainEqual(cell(3, 8));
  expect(new Set(state.selected.map(cellKey)).size).toBe(3);
});

test('Shift keeps its anchor through repeated extensions and Ctrl+Shift adds a rectangle', () => {
  const initial = cell(0, 2);
  const extended = selectCell([initial], initial, cell(3, 2), rows, frames, { range: true });
  expect(extended.anchor).toEqual(initial);
  expect(selectCell(extended.selected, extended.anchor, cell(0, 4), rows, frames, { range: true }).selected).toHaveLength(3);
  const added = selectCell([cell(3, 9)], initial, cell(1, 3), rows, frames, { range: true, additive: true });
  expect(added.selected).toHaveLength(5);
});

test('dragging replaces a range or adds to a captured selection without accumulating stale rectangles', () => {
  const start = cell(1, 1), prior = [cell(3, 9)];
  expect(dragCells(prior, start, cell(2, 3), rows, frames, false)).toHaveLength(6);
  expect(dragCells(prior, start, cell(2, 3), rows, frames, true)).toHaveLength(7);
  expect(dragCells(prior, start, cell(1, 2), rows, frames, true)).toHaveLength(3);
});

test('cell groups keep only selected destinations in timeline order', () => {
  expect(groupCells([cell(2, 9), cell(1, 6), cell(1, 1), cell(3, 8)], frames)).toEqual([
    { layerId: 'body', frameIds: ['frame-10'] },
    { layerId: 'face', frameIds: ['frame-2', 'frame-7'] },
    { layerId: 'background', frameIds: ['frame-9'] },
  ]);
});
