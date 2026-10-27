import { expect, test } from 'bun:test';
import { directionalCopies, moveHeader } from '../../src/editor/timeline/matrixActions';
const frames = ['one', 'two', 'three', 'four'];
test('directional copies read original sources rather than cascading over a selected range', () => {
  const cells = ['one','two','three'].map(frameId=>({layerId:'ink',frameId}));
  const right = directionalCopies(cells, frames, 1);
  expect(right.map(op=>op.source_frame_id)).toEqual(['three','two','one']);
  expect(right.map(op=>op.frame_ids[0])).toEqual(['four','three','two']);
  expect(directionalCopies(cells,frames,-1).map(op=>op.source_frame_id)).toEqual(['two','three']);
});
test('directional copies preserve sparse selections, layer identities, and timeline boundaries', () => {
  const ops=directionalCopies([{layerId:'face',frameId:'two'},{layerId:'body',frameId:'four'}],frames,1);
  expect(ops).toEqual([{type:'copy_cells',layer_id:'face',source_frame_id:'two',frame_ids:['three']}]);
  expect(directionalCopies([{layerId:'face',frameId:'one'}],frames,-1)).toEqual([]);
});
test('moving a header keeps every other row/column in order and supports both insertion sides', () => {
  expect(moveHeader(frames,'one','three',true)).toEqual(['two','three','one','four']);
  expect(moveHeader(frames,'four','two',false)).toEqual(['one','four','two','three']);
  expect(moveHeader(frames,'two','two',true)).toEqual(frames);
  expect(moveHeader(frames,'missing','two',false)).toEqual(frames);
});
