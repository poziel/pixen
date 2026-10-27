import type { Point } from './raster';
/** Tool-independent click tracing. Consumers decide how a finished outline is used. */
export class PolygonTrace {
  constructor(private closeOnStart=true){}
  points:Point[]=[];
  cursor:Point|null=null;
  closable=false;
  private last:{point:Point;time:number;button:number}|null=null;
  move(point:Point,radius:number) {
    this.cursor=point;
    this.closable=this.points.length>=3&&Math.hypot(point.x-this.points[0].x,point.y-this.points[0].y)<=radius;
  }
  get preview():Point[] {
    if(!this.points.length)return [];
    return this.cursor?[...this.points,this.closable?this.points[0]:this.cursor]:[...this.points];
  }
  place(point:Point,time:number,radius:number,button=0):'vertex'|'close'|'ignored' {
    this.move(point,radius);
    const double=this.last&&this.last.button===button&&time-this.last.time<400&&Math.hypot(point.x-this.last.point.x,point.y-this.last.point.y)<=radius;
    this.last={point:{...point},time,button};
    if(!this.points.length&&double)return 'ignored';
    if((this.closeOnStart?this.points.length>=3:this.points.length>=2)&&((this.closeOnStart&&this.closable)||double))return 'close';
    if(!this.points.length||point.x!==this.points.at(-1)!.x||point.y!==this.points.at(-1)!.y)this.points.push({...point});
    return 'vertex';
  }
  reset(preserveClick=false){this.points=[];this.cursor=null;this.closable=false;if(!preserveClick)this.last=null;}
}
