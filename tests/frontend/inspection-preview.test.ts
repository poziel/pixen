import {expect,test} from 'bun:test';
import {inspectionPreview} from '../../src/editor/tools/inspectionPreview';
test('onion ghosts remain behind current artwork and never change its source pixels',()=>{
 const current=new Uint8ClampedArray([10,20,30,255,0,0,0,0]);const before=current.slice();
 const prior=new Uint8ClampedArray([255,255,255,255,255,255,255,255]);
 const output=inspectionPreview(current,'onion_skin',prior);
 expect([...output.slice(0,4)]).toEqual([10,20,30,255]);expect([...output.slice(4)]).toEqual([255,80,80,90]);expect(current).toEqual(before);
});
test('comparison changes only the preview and uses the original RGBA for equality',()=>{
 const current=new Uint8ClampedArray([1,2,3,255,10,20,30,255]);
 const prior=new Uint8ClampedArray([1,2,3,255,0,0,0,0]);
 expect([...inspectionPreview(current,'frame_comparison',prior)]).toEqual([1,2,3,60,255,64,180,255]);expect(current[4]).toBe(10);
});
