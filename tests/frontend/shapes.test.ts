import { expect,test } from 'bun:test';
import { lineEnds, shapePoints, ShapeDraft } from '../../src/editor/tools/shapes';
test('line modifiers mirror about origin and snap every 22.5 degrees',()=>{
 expect(lineEnds({x:5,y:5},{x:9,y:6},true,false)).toEqual([{x:1,y:4},{x:9,y:6}]);
 const [a,b]=lineEnds({x:10,y:10},{x:20,y:11},false,true);expect(a).toEqual({x:10,y:10});expect(b.y).toBe(10);
 const [c,d]=lineEnds({x:10,y:10},{x:20,y:14},true,true);expect(c.x+d.x).toBe(20);expect(c.y+d.y).toBe(20);
 for(let i=0;i<16;i++){const angle=i*Math.PI/8,p={x:Math.cos(angle+.03)*100,y:Math.sin(angle+.03)*100};const [,end]=lineEnds({x:0,y:0},p,false,true);expect(Math.abs(end.x-Math.cos(angle)*100)).toBeLessThan(1);expect(Math.abs(end.y-Math.sin(angle)*100)).toBeLessThan(1);}
});
test('shape rasterizers keep rectangles and ellipses hollow and produce twelve box edges',()=>{
 const rectangle=shapePoints('draw-rectangle',[{x:1,y:1},{x:5,y:4}],[],false,false,16,16);
 expect(rectangle.some(p=>p.x===3&&p.y===2)).toBe(false);expect(rectangle).toContainEqual({x:1,y:1});
 const ellipse=shapePoints('draw-ellipse',[{x:2,y:2},{x:8,y:8}],[],false,false,16,16);expect(ellipse.some(p=>p.x===5&&p.y===5)).toBe(false);
 const box=shapePoints('isometric',[{x:6,y:7},{x:10,y:9},{x:2,y:9},{x:6,y:3}],[],false,false,16,16);expect(box).toContainEqual({x:6,y:3});expect(box).toContainEqual({x:10,y:5});
});
test('curves have live endpoints, tangent dragging, point removal and double-click completion',()=>{
 const draft=new ShapeDraft('curve',{x:1,y:1},0);
 draft.move({x:3,y:0},{shift:false,ctrl:false,alt:false},true);draft.up();expect(draft.tangents[0]).toEqual({x:2,y:-1});
 draft.move({x:6,y:5},{shift:false,ctrl:false,alt:false},false);expect(draft.preview).toHaveLength(2);
 draft.down({x:6,y:5},500);draft.up();expect(draft.down({x:6,y:5},600)).toBe(true);
 draft.removeLast();expect(draft.points).toHaveLength(1);
});
test('Alt displaces the draft origin without changing its dimensions',()=>{
 const draft=new ShapeDraft('line',{x:2,y:2},0);
 draft.move({x:6,y:4},{shift:false,ctrl:false,alt:false},true);
 draft.move({x:8,y:5},{shift:false,ctrl:false,alt:true},true);
 expect(draft.points[0]).toEqual({x:4,y:3});expect(draft.preview[1]).toEqual({x:8,y:5});
});
