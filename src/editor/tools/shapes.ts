import { line, selectionMask, type Point } from './raster';
import { placementPath } from './selection';
import { PolygonTrace } from './polygonTrace';
export const shapeTools=['line','curve','draw-rectangle','draw-ellipse','isometric'] as const;
export type ShapeTool=typeof shapeTools[number];
export type ShapeModifiers={shift:boolean;ctrl:boolean;alt:boolean};
export function isShape(tool:string):tool is ShapeTool{return (shapeTools as readonly string[]).includes(tool);}
export function lineEnds(a:Point,b:Point,centered:boolean,snap:boolean):[Point,Point]{
 let dx=b.x-a.x,dy=b.y-a.y;
 if(snap){const angle=Math.round(Math.atan2(dy,dx)/(Math.PI/8))*Math.PI/8,length=Math.hypot(dx,dy);dx=Math.round(Math.cos(angle)*length);dy=Math.round(Math.sin(angle)*length);}
 return [centered?{x:a.x-dx,y:a.y-dy}:{...a},{x:a.x+dx,y:a.y+dy}];
}
export function shapePoints(tool:ShapeTool,points:Point[],tangents:Point[],shift:boolean,ctrl:boolean,width:number,height:number):Point[]{
 if(!points.length)return [];
 const result:Point[]=[];
 const segment=(a:Point,b:Point)=>result.push(...line({x:Math.round(a.x),y:Math.round(a.y)},{x:Math.round(b.x),y:Math.round(b.y)}));
 if(tool==='line'){const ends=lineEnds(points[0],points.at(-1)!,ctrl,shift);segment(...ends);}
 if(tool==='draw-rectangle'||tool==='draw-ellipse'){
  const [a,b]=placementPath([points[0],points.at(-1)!],tool==='draw-rectangle'?'rectangle':'ellipse',shift,ctrl);
  if(tool==='draw-rectangle'){segment(a,{x:b.x,y:a.y});segment({x:b.x,y:a.y},b);segment(b,{x:a.x,y:b.y});segment({x:a.x,y:b.y},a);}
  else {
   // Evaluate the ellipse within its own bounds, retaining off-artboard preview pixels.
   const w=b.x-a.x+1,h=b.y-a.y+1;
   const mask=selectionMask('ellipse',w,h,[{x:0,y:0},{x:w-1,y:h-1}]);
   for(const n of mask){const x=n%w,y=Math.floor(n/w);if(x===0||x===w-1||y===0||y===h-1||!mask.has(n-1)||!mask.has(n+1)||!mask.has(n-w)||!mask.has(n+w))result.push({x:x+a.x,y:y+a.y});}
  }
 }
 if(tool==='curve'){
  if(points.length===1)return [{...points[0]}];
  for(let i=1;i<points.length;i++){
   const a=points[i-1],b=points[i],ta=tangents[i-1]??{x:0,y:0},tb=tangents[i]??{x:0,y:0};
   const c={x:a.x+ta.x,y:a.y+ta.y},d={x:b.x-tb.x,y:b.y-tb.y};
   const steps=Math.max(1,Math.min(16384,Math.ceil((Math.hypot(c.x-a.x,c.y-a.y)+Math.hypot(d.x-c.x,d.y-c.y)+Math.hypot(b.x-d.x,b.y-d.y))*3)));
   let previous=a;
   for(let j=1;j<=steps;j++){const t=j/steps,u=1-t,p={x:u*u*u*a.x+3*u*u*t*c.x+3*u*t*t*d.x+t*t*t*b.x,y:u*u*u*a.y+3*u*u*t*c.y+3*u*t*t*d.y+t*t*t*b.y};segment(previous,p);previous=p;}
  }
 }
 if(tool==='isometric'){
  const o=points[0];if(points.length===1)return [o];
  const a=points[1];segment(o,a);
  if(points.length>=3){const b=points[2],far={x:a.x+b.x-o.x,y:a.y+b.y-o.y},base=[o,a,far,b];segment(o,b);segment(a,far);segment(b,far);
   if(points.length>=4){const delta={x:points[3].x-o.x,y:points[3].y-o.y},top=base.map(p=>({x:p.x+delta.x,y:p.y+delta.y}));for(let i=0;i<4;i++){segment(base[i],top[i]);segment(top[i],top[(i+1)%4]);}}
  }
 }
 const unique=new Map<string,Point>();for(const p of result)unique.set(`${p.x},${p.y}`,p);
 return [...unique.values()];
}
/** Staged tracing shares vertex/double-click detection with polygon selections. */
export class ShapeDraft {
 readonly trace=new PolygonTrace(false);
 readonly tangents:Point[]=[];
 modifiers:ShapeModifiers={shift:false,ctrl:false,alt:false};
 private hover:Point;
 private previous:Point;
 private dragging=false;
 constructor(readonly tool:ShapeTool,start:Point,time:number){this.trace.place(start,time,.75);this.tangents.push({x:0,y:0});this.hover={...start};this.previous={...start};this.dragging=true;}
 get points(){return this.trace.points;}
 get staged(){return this.tool==='curve'||this.tool==='isometric';}
 get preview():Point[]{
  if(!this.staged)return [this.points[0],this.hover];
  if(this.dragging||this.points.length>=4&&this.tool==='isometric')return this.points;
  const last=this.points.at(-1);return last&&last.x===this.hover.x&&last.y===this.hover.y?this.points:[...this.points,this.hover];
 }
 down(p:Point,time:number){
  if(this.tool==='curve'&&this.points.length>=2&&this.trace.place(p,time,.75)==='close')return true;
  if(this.tool==='isometric'||this.tool==='curve'&&this.points.length<2)this.trace.place(p,time,.75);
  if(this.tangents.length<this.points.length)this.tangents.push({x:0,y:0});
  this.hover={...p};this.previous={...p};this.dragging=true;
  return this.tool==='isometric'&&this.points.length>=4;
 }
 move(p:Point,modifiers:ShapeModifiers,pressed:boolean){
  this.modifiers=modifiers;
  if(modifiers.alt){const dx=p.x-this.previous.x,dy=p.y-this.previous.y;this.points.forEach(v=>{v.x+=dx;v.y+=dy;});this.hover={x:this.hover.x+dx,y:this.hover.y+dy};}
  else if(this.tool==='curve'&&this.dragging&&pressed){const a=this.points.at(-1)!;this.tangents[this.points.length-1]={x:p.x-a.x,y:p.y-a.y};this.hover={...a};}
  else {
   this.hover={...p};
   if(this.tool==='isometric'&&modifiers.shift){const o=this.points[0],dx=p.x-o.x,dy=p.y-o.y;if(this.points.length>=3)this.hover.x=o.x;else{const angles=[0,Math.atan(.5),Math.PI/2,Math.PI-Math.atan(.5),Math.PI,Math.PI+Math.atan(.5),Math.PI*1.5,Math.PI*2-Math.atan(.5)],angle=Math.atan2(dy,dx),snap=angles.reduce((a,b)=>Math.abs(Math.atan2(Math.sin(angle-a),Math.cos(angle-a)))<Math.abs(Math.atan2(Math.sin(angle-b),Math.cos(angle-b)))?a:b);this.hover={x:o.x+Math.round(Math.cos(snap)*Math.hypot(dx,dy)),y:o.y+Math.round(Math.sin(snap)*Math.hypot(dx,dy))};}}
  }
  this.previous={...p};
 }
 get endpoint(){return this.hover;}
 up(){this.dragging=false;return !this.staged||this.tool==='isometric'&&this.points.length>=4;}
 removeLast(){if(this.points.length>1){this.points.pop();this.tangents.pop();this.dragging=false;}}
}
