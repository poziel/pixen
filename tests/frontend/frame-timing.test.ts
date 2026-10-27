import {expect,test} from 'bun:test';
import {durationOperations,previousFrameCopies} from '../../src/editor/timeline/frameTiming';
test('duration edits deduplicate selected frame IDs and reject invalid input before mutation',()=>{
 expect(durationOperations(['a','b','a'],55)).toEqual([{type:'set_frame_duration',frame_id:'a',duration_ms:55},{type:'set_frame_duration',frame_id:'b',duration_ms:55}]);
 for(const value of [0,-1,NaN,Infinity])expect(()=>durationOperations(['a'],value)).toThrow();
});
test('copy previous frame fills every layer in an existing column without inserting or changing timing',()=>{
 expect(previousFrameCopies(['body','face'],['one','two','three'],'two')).toEqual([
  {type:'copy_cells',layer_id:'body',source_frame_id:'one',frame_ids:['two']},
  {type:'copy_cells',layer_id:'face',source_frame_id:'one',frame_ids:['two']},
 ]);
 expect(previousFrameCopies(['body'],['one','two'],'one')).toEqual([]);
});
