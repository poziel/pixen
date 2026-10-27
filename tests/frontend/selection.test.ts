import { expect, test } from 'bun:test';
import { resizeBounds, transformMask, maskBounds, combineSelection, selectionMode, boundaryPath, placementPath, beginModifiers, updateModifiers } from '../../src/editor/tools/selection';
import { selectionMask } from '../../src/editor/tools/raster';

const box = { x: 10, y: 20, width: 20, height: 10 };
test('edge and corner resizing anchor the opposite edges', () => {
  expect(resizeBounds(box, 'e', 5, 4, false, false)).toEqual({ ...box, width: 25 });
  expect(resizeBounds(box, 'nw', -5, -4, false, false)).toEqual({ x: 5, y: 16, width: 25, height: 14 });
});
test('Shift preserves aspect ratio at each side and corner', () => {
  expect(resizeBounds(box, 'e', 20, 0, true, false)).toEqual({ x: 10, y: 15, width: 40, height: 20 });
  expect(resizeBounds(box, 'w', -20, 0, true, false)).toEqual({ x: -10, y: 15, width: 40, height: 20 });
  expect(resizeBounds(box, 'n', 0, -10, true, false)).toEqual({ x: 0, y: 10, width: 40, height: 20 });
  expect(resizeBounds(box, 's', 0, 10, true, false)).toEqual({ x: 0, y: 20, width: 40, height: 20 });
  expect(resizeBounds(box, 'nw', -20, -10, true, false)).toEqual({ x: -10, y: 10, width: 40, height: 20 });
});
test('Ctrl+Shift expands proportionally about the original center', () => {
  expect(resizeBounds(box, 'e', 10, 0, true, true)).toEqual({ x: 0, y: 15, width: 40, height: 20 });
  expect(resizeBounds(box, 'nw', -10, -5, true, true)).toEqual({ x: 0, y: 15, width: 40, height: 20 });
  expect(resizeBounds(box, 'w', 100, 0, true, true).width).toBeGreaterThanOrEqual(1);
});
test('Control mirrors the dragged edges freely without locking proportions', () => {
  expect(resizeBounds(box, 'e', 5, 0, false, true)).toEqual({ x: 5, y: 20, width: 30, height: 10 });
  expect(resizeBounds(box, 'w', -5, 0, false, true)).toEqual({ x: 5, y: 20, width: 30, height: 10 });
  expect(resizeBounds(box, 'n', 0, -3, false, true)).toEqual({ x: 10, y: 17, width: 20, height: 16 });
  expect(resizeBounds(box, 's', 0, 3, false, true)).toEqual({ x: 10, y: 17, width: 20, height: 16 });
  expect(resizeBounds(box, 'nw', -5, -3, false, true)).toEqual({ x: 5, y: 17, width: 30, height: 16 });
});
test('mask transforms preserve holes, polygon shape, and disconnected components', () => {
  const mask = new Set([0, 2, 8, 10]);
  const enlarged = transformMask(mask, 8, 8, { x: 0, y: 0, width: 3, height: 2 }, { x: 1, y: 2, width: 6, height: 4 });
  expect(enlarged.size).toBe(16);
  expect(enlarged.has(2 * 8 + 3)).toBe(false);
  expect(maskBounds(enlarged, 8)).toEqual({ x: 1, y: 2, width: 6, height: 4 });
  const polygon = selectionMask('polygon', 16, 16, [{x:1,y:1},{x:5,y:1},{x:1,y:5}]);
  const shifted = transformMask(polygon, 16, 16, maskBounds(polygon, 16)!, { ...maskBounds(polygon, 16)!, x: 4, y: 4 });
  expect(shifted.size).toBe(polygon.size);
  expect([...shifted].every(n => polygon.has(n - 3 - 3 * 16))).toBe(true);
});
test('combining masks and modifiers supports empty selections and temporary overrides', () => {
  expect([...combineSelection(new Set([1,2]),new Set([2,3]),'add')]).toEqual([1,2,3]);
  expect([...combineSelection(new Set([1,2]),new Set([2,3]),'subtract')]).toEqual([1]);
  expect(combineSelection(null,new Set([1]),'subtract').size).toBe(0);
  expect(selectionMode('replace',true,false)).toBe('add');
  expect(selectionMode('add',true,true)).toBe('subtract');
  expect(selectionMode('subtract',false,false)).toBe('subtract');
  expect(boundaryPath(new Set([0,1]),4)).not.toContain('M1 0v1');
});
test('Control before drawing intersects instead of replacing or moving the existing mask', () => {
  expect(selectionMode('replace',false,true)).toBe('intersect');
  expect([...combineSelection(new Set([1,2]),new Set([2,3]),'intersect')]).toEqual([2]);
  expect(combineSelection(null,new Set([2]),'intersect').size).toBe(0);
});
test('held starting modifiers choose the operation; later presses control geometry', () => {
  const modifiers=beginModifiers(true,false);
  updateModifiers(modifiers,true,false);
  expect(modifiers.proportional).toBe(false);
  updateModifiers(modifiers,false,false);
  updateModifiers(modifiers,true,true);
  expect(modifiers.proportional).toBe(true);
  expect(modifiers.centered).toBe(true);
});
test('new rectangular and elliptical bounds support all four geometry combinations', () => {
  const path=[{x:5,y:5},{x:7,y:6}];
  expect(placementPath(path,'rectangle',false,false)).toEqual(path);
  expect(placementPath(path,'rectangle',true,false)).toEqual([{x:5,y:5},{x:7,y:7}]);
  expect(placementPath(path,'ellipse',false,true)).toEqual([{x:3,y:4},{x:7,y:6}]);
  expect(placementPath(path,'ellipse',true,true)).toEqual([{x:3,y:3},{x:7,y:7}]);
  expect(placementPath([{x:5,y:5},{x:3,y:4}],'rectangle',true,false)).toEqual([{x:3,y:3},{x:5,y:5}]);
});
test('polygon and lasso placement transforms preserve vertices and center their bounds', () => {
  const path=[{x:2,y:2},{x:6,y:2},{x:4,y:4}];
  for(const kind of ['polygon','lasso'] as const){
    expect(placementPath(path,kind,true,false)).toEqual([{x:2,y:2},{x:6,y:2},{x:4,y:6}]);
    expect(placementPath(path,kind,false,true)).toEqual([{x:0,y:1},{x:4,y:1},{x:2,y:3}]);
  }
});
test('selection preview outlines can extend outside while committed masks stay clipped', () => {
  const mask=new Set([0,1,4,5]),from={x:0,y:0,width:2,height:2},to={x:-3,y:-2,width:2,height:2};
  expect(boundaryPath(mask,4,{from,to})).toContain('M-3 -2h1');
  expect(transformMask(mask,4,4,from,to).size).toBe(0);
  expect(mask.size).toBe(4);
});
