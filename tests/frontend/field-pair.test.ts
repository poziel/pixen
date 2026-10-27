import { expect, test } from 'bun:test';
import { updatePair } from '../../src/editor/tools/fieldPair';

test('linked size fields preserve the captured ratio in both directions', () => {
  expect(updatePair([50,26],0,100,[50,26],1)).toEqual([100,52]);
  expect(updatePair([50,26],1,52,[50,26],1)).toEqual([100,52]);
  expect(updatePair([3,2],0,4,[3,2],1)).toEqual([4,3]);
  expect(updatePair([4,3],0,6,[3,2],1)).toEqual([6,4]);
});
test('unlinked pairs change only the edited field and respect integer bounds', () => {
  expect(updatePair([50,26],0,100,null,1)).toEqual([100,26]);
  expect(updatePair([10,20],1,-5,null,0)).toEqual([10,0]);
  expect(updatePair([50,26],0,0,[50,26],1)).toEqual([1,1]);
  expect(updatePair([50,26],0,NaN,[50,26],1)).toEqual([50,26]);
});
test('linked gaps initially at zero can increase together',()=>{
 expect(updatePair([0,0],0,3,[0,0],0)).toEqual([3,3]);
});
