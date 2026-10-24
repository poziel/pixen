import {expect,test} from 'bun:test';
import {zoomPan} from '../../src/editor/canvas/zoom';
test('zoom keeps the same artwork coordinate at an off-center cursor after panning',()=>{
 const pan={x:37,y:-19},anchor={x:215,y:-143},oldScale=16.37,newScale=oldScale*1.15;
 const next=zoomPan(pan,anchor,newScale/oldScale);
 expect((anchor.x-next.x)/newScale).toBeCloseTo((anchor.x-pan.x)/oldScale,10);
 expect((anchor.y-next.y)/newScale).toBeCloseTo((anchor.y-pan.y)/oldScale,10);
 const restored=zoomPan(next,anchor,oldScale/newScale);
 expect(restored.x).toBeCloseTo(pan.x,10);expect(restored.y).toBeCloseTo(pan.y,10);
});
test('status zoom anchors at viewport center, including after panning',()=>{
 expect(zoomPan({x:20,y:-30},{x:0,y:0},2)).toEqual({x:40,y:-60});
});
