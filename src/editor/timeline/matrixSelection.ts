export interface MatrixCell { layerId: string; frameId: string }
export const cellKey = (cell: MatrixCell) => JSON.stringify([cell.layerId, cell.frameId]);

export function rectangleCells(from: MatrixCell, to: MatrixCell, rows: readonly string[], frames: readonly string[]): MatrixCell[] {
  const a = rows.indexOf(from.layerId), b = rows.indexOf(to.layerId);
  const x = frames.indexOf(from.frameId), y = frames.indexOf(to.frameId);
  if (Math.min(a, b, x, y) < 0) return [];
  return rows.slice(Math.min(a, b), Math.max(a, b) + 1).flatMap(layerId =>
    frames.slice(Math.min(x, y), Math.max(x, y) + 1).map(frameId => ({ layerId, frameId })));
}
function union(first: readonly MatrixCell[], second: readonly MatrixCell[]) {
  return [...new Map([...first, ...second].map(cell => [cellKey(cell), cell])).values()];
}
export function dragCells(base: readonly MatrixCell[], anchor: MatrixCell, target: MatrixCell, rows: readonly string[], frames: readonly string[], additive: boolean) {
  const range = rectangleCells(anchor, target, rows, frames);
  return additive ? union(base, range) : range;
}
export function selectCell(selected: readonly MatrixCell[], anchor: MatrixCell | undefined, target: MatrixCell, rows: readonly string[], frames: readonly string[], modifiers: { range?: boolean; additive?: boolean } = {}) {
  if (modifiers.range && anchor && rows.includes(anchor.layerId) && frames.includes(anchor.frameId)) {
    return { selected: dragCells(selected, anchor, target, rows, frames, !!modifiers.additive), anchor };
  }
  const key = cellKey(target);
  return { selected: modifiers.additive
    ? selected.some(cell => cellKey(cell) === key) ? selected.filter(cell => cellKey(cell) !== key) : [...selected, target]
    : [target], anchor: target };
}
export function groupCells(selected: readonly MatrixCell[], frames: readonly string[]) {
  const groups = new Map<string, Set<string>>();
  for (const cell of selected) {
    if (!groups.has(cell.layerId)) groups.set(cell.layerId, new Set());
    groups.get(cell.layerId)!.add(cell.frameId);
  }
  return [...groups].map(([layerId, ids]) => ({ layerId, frameIds: frames.filter(id => ids.has(id)) }));
}
