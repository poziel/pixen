import { expect, test } from 'bun:test';
import { PolygonTrace } from '../../src/editor/tools/polygonTrace';
test('tracing previews a moving endpoint and closes when returning to the start',()=>{
  const trace=new PolygonTrace();
  trace.place({x:1,y:1},0,1);
  trace.move({x:5,y:2},1);
  expect(trace.preview).toEqual([{x:1,y:1},{x:5,y:2}]);
  expect(trace.points).toHaveLength(1);
  trace.place({x:6,y:1},500,1);trace.place({x:6,y:6},1000,1);
  trace.move({x:1.2,y:1.1},1);
  expect(trace.closable).toBe(true);
  expect(trace.place({x:1,y:1},1500,1)).toBe('close');
  expect(trace.points).toHaveLength(3);
});
test('double click closes without duplicate vertices and cancel clears preview',()=>{
  const trace=new PolygonTrace();
  trace.place({x:0,y:0},0,.2);trace.place({x:4,y:0},500,.2);trace.place({x:4,y:4},1000,.2);
  expect(trace.place({x:4,y:4},1100,.2)).toBe('close');
  expect(trace.points).toHaveLength(3);
  trace.reset();expect(trace.preview).toEqual([]);expect(trace.closable).toBe(false);
});
