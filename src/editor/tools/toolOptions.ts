import type { Brush } from './raster';
import { shapeTools, type ShapeTool } from './shapes';
export function createToolOptions(slot:'left'|'right') {
 const shapes=Object.fromEntries(shapeTools.map(id=>[id,{size:1,density:100,opacity:100,shape:'square',filled:true}])) as Record<ShapeTool,Brush>;
 return {
  shapes,
  zoom:(slot==='left'?'in':'out') as 'in'|'out',
  selectionBrush:{size:5,shape:'square' as 'square'|'circle',action:'select' as 'select'|'deselect'},
  color:{tolerance:0,merged:false},wand:{tolerance:0,merged:false},
  bucket:{tolerance:0,area:'contiguous' as 'contiguous'|'matching'|'selection',merged:false,fill:'color' as 'color'|'transparent'},
  picker:{target:'button' as 'button'|'primary'|'secondary',source:'layer' as 'layer'|'composite',channels:'rgba' as 'rgba'|'rgb'|'alpha'},
  text:{content:'Text',size:8,font:'monospace',bold:false,italic:false,align:'left' as 'left'|'center'|'right'|'justify',antialias:false},
  move:{copy:false},
  paper:{opacity:100},
 };
}
export type ToolOptions=ReturnType<typeof createToolOptions>;
