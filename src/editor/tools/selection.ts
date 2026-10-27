export type SelectionBounds = { x: number; y: number; width: number; height: number };
import type { Point } from './raster';
export type SelectionMode = 'replace' | 'add' | 'subtract' | 'intersect';
export type SelectionHandle = 'nw'|'n'|'ne'|'e'|'se'|'s'|'sw'|'w';
export const selectionHandles: SelectionHandle[] = ['nw','n','ne','e','se','s','sw','w'];
export function handlePoint(b: SelectionBounds, handle: SelectionHandle) {
  return { x: b.x + (handle.includes('w') ? 0 : handle.includes('e') ? b.width : b.width / 2),
    y: b.y + (handle.includes('n') ? 0 : handle.includes('s') ? b.height : b.height / 2) };
}
export function selectionMode(base: SelectionMode, shift: boolean, control: boolean): SelectionMode {
  return shift ? control ? 'subtract' : 'add' : control ? 'intersect' : base;
}
export function combineSelection(current: Set<number> | null, next: Set<number>, mode: SelectionMode) {
  if (mode === 'replace') return new Set(next);
  if (mode === 'intersect') return new Set([...next].filter(n=>current?.has(n)));
  const result = new Set(current);
  for (const n of next) { if (mode === 'add') result.add(n); else result.delete(n); }
  return result;
}
export type PlacementModifiers = { blockedShift:boolean; blockedControl:boolean; proportional:boolean; centered:boolean };
/** Starting keys belong to the selection operation until released and pressed again. */
export function beginModifiers(shift:boolean,control:boolean):PlacementModifiers {
  return {blockedShift:shift,blockedControl:control,proportional:false,centered:false};
}
export function updateModifiers(state:PlacementModifiers,shift:boolean,control:boolean) {
  if(!shift)state.blockedShift=false;
  if(!control)state.blockedControl=false;
  state.proportional=shift&&!state.blockedShift;
  state.centered=control&&!state.blockedControl;
}
/** Shared placement geometry for selection boundaries and future shape tools. */
export function placementPath(path:Point[],kind:'rectangle'|'ellipse'|'polygon'|'lasso',proportional:boolean,centered:boolean):Point[] {
  if(!path.length||(!proportional&&!centered))return path;
  const first=path[0],last=path[path.length-1];
  if(kind==='rectangle'||kind==='ellipse'){
    let dx=Math.abs(last.x-first.x),dy=Math.abs(last.y-first.y);
    if(proportional)dx=dy=Math.max(dx,dy);
    return [{x:centered?first.x-dx:last.x<first.x?first.x-dx:first.x,y:centered?first.y-dy:last.y<first.y?first.y-dy:first.y},
      {x:centered?first.x+dx:last.x<first.x?first.x:first.x+dx,y:centered?first.y+dy:last.y<first.y?first.y:first.y+dy}];
  }
  // Freeform and polygon masks retain their vertices while their bounding box transforms.
  let left=Infinity,top=Infinity,right=-Infinity,bottom=-Infinity;
  for(const p of path){left=Math.min(left,p.x);top=Math.min(top,p.y);right=Math.max(right,p.x);bottom=Math.max(bottom,p.y);}
  const width=Math.max(1,right-left),height=Math.max(1,bottom-top);
  const size=Math.max(width,height),w=proportional?size:width,h=proportional?size:height;
  const x=centered?first.x-w/2:left,y=centered?first.y-h/2:top;
  return path.map(p=>({x:x+(p.x-left)*w/width,y:y+(p.y-top)*h/height}));
}
export function maskBounds(mask: Set<number> | null, width: number): SelectionBounds | null {
  if (!mask?.size) return null;
  let x = Infinity, y = Infinity, right = 0, bottom = 0;
  for (const n of mask) { const px = n % width, py = Math.floor(n / width); x = Math.min(x,px); y = Math.min(y,py); right = Math.max(right,px+1); bottom = Math.max(bottom,py+1); }
  return { x,y,width:right-x,height:bottom-y };
}
export function resizeBounds(b: SelectionBounds, handle: SelectionHandle, dx: number, dy: number, proportional: boolean, centered: boolean): SelectionBounds {
  const horizontal = handle.includes('w') || handle.includes('e'), vertical = handle.includes('n') || handle.includes('s');
  const factor = centered ? 2 : 1;
  let width = Math.max(1, b.width + (horizontal ? dx * (handle.includes('w') ? -1 : 1) * factor : 0));
  let height = Math.max(1, b.height + (vertical ? dy * (handle.includes('n') ? -1 : 1) * factor : 0));
  if (proportional) {
    const sx = width / b.width, sy = height / b.height;
    const ratio = horizontal && vertical ? Math.abs(sx-1) >= Math.abs(sy-1) ? sx : sy : horizontal ? sx : sy;
    width = Math.max(1, Math.round(b.width * ratio)); height = Math.max(1, Math.round(b.height * ratio));
  }
  width = Math.round(width); height = Math.round(height);
  const x = centered || !horizontal ? b.x + (b.width-width)/2 : handle.includes('w') ? b.x+b.width-width : b.x;
  const y = centered || !vertical ? b.y + (b.height-height)/2 : handle.includes('n') ? b.y+b.height-height : b.y;
  return {x:Math.round(x),y:Math.round(y),width,height};
}
/** Nearest-neighbor inverse sampling transforms the mask, never the artwork. */
export function transformMask(mask: Set<number>, width: number, height: number, from: SelectionBounds, to: SelectionBounds) {
  const result = new Set<number>();
  for (let y = Math.max(0,to.y); y < Math.min(height,to.y+to.height); y++)
    for (let x = Math.max(0,to.x); x < Math.min(width,to.x+to.width); x++) {
      const sx = from.x + Math.min(from.width-1,Math.floor((x-to.x+.5)*from.width/to.width));
      const sy = from.y + Math.min(from.height-1,Math.floor((y-to.y+.5)*from.height/to.height));
      if (mask.has(sy*width+sx)) result.add(y*width+x);
    }
  return result;
}
export function boundaryPath(mask: Set<number> | null, width: number, transform?:{from:SelectionBounds;to:SelectionBounds}) {
  if (!mask) return '';
  const parts: string[] = [];
  for (const n of mask) {
    const px=n%width,py=Math.floor(n/width),sx=transform?transform.to.width/transform.from.width:1,sy=transform?transform.to.height/transform.from.height:1;
    const x=transform?transform.to.x+(px-transform.from.x)*sx:px,y=transform?transform.to.y+(py-transform.from.y)*sy:py;
    if (!mask.has(n-width)) parts.push(`M${x} ${y}h${sx}`);
    if (!mask.has(n+width)) parts.push(`M${x} ${y+sy}h${sx}`);
    if (px===0||!mask.has(n-1)) parts.push(`M${x} ${y}v${sy}`);
    if (px===width-1||!mask.has(n+1)) parts.push(`M${x+sx} ${y}v${sy}`);
  }
  return parts.join('');
}
