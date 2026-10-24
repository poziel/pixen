import type { Point, Tool, Brush } from './raster';

export type ToolSlot = 'left' | 'right';
export function createBrushSettings(): Record<'pencil' | 'eraser', Brush> {
  return {
    pencil: {size:1,density:100,opacity:100},
    eraser: {size:1,density:100,opacity:100},
  };
}

export function footprint(tool: Tool, point: Point, brushSize: number) {
  if (tool === 'pan' || tool === 'zoom') return null;
  const size = tool === 'pencil' || tool === 'eraser'
    ? Math.max(1, Math.min(256, Math.round(brushSize) || 1)) : 1;
  return {x:point.x-Math.floor(size/2),y:point.y-Math.floor(size/2),size};
}

export function resizedToolValue(tool: Tool, current: number, deltaY: number) {
  if (tool !== 'pencil' && tool !== 'eraser' && tool !== 'text') return null;
  return Math.max(1, Math.min(tool === 'text' ? 128 : 256,
    (Math.round(current) || 1) + (deltaY < 0 ? 1 : deltaY > 0 ? -1 : 0)));
}
