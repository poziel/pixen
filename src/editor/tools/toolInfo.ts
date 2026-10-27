import type {Tool} from './raster';
export const selectionTools:readonly Tool[]=['rectangle','ellipse','polygon','lasso','paint-select','color','wand'];
export function isSelectionTool(tool:Tool){return selectionTools.includes(tool);}
export const toolInfo:Record<Tool,{key?:string;description:string;detail?:string}>={
 rectangle:{key:'r',description:'Draw a rectangular selection.',detail:'Before drawing: `Shift` to add, `Control` to intersect, `Control + Shift` to subtract.\nDuring drawing: `Shift` to constrain, `Control` to center, `Control + Shift` for both.'},
 ellipse:{key:'m',description:'Draw an elliptical selection.',detail:'Before drawing: `Shift` to add, `Control` to intersect, `Control + Shift` to subtract.\nDuring drawing: `Shift` for a circle, `Control` to center, `Control + Shift` for both.'},
 polygon:{key:'k',description:'Click vertices and preview the next edge.',detail:'Double-click or click the highlighted start point to finish. `Escape` cancels.\nBefore the first click: `Shift` to add, `Control` to intersect, `Control + Shift` to subtract.'},
 lasso:{key:'f',description:'Trace a freehand selection. Release to close its boundary.',detail:'Before drawing: `Shift` to add, `Control` to intersect, `Control + Shift` to subtract.'},
 'paint-select':{key:'i',description:'Paint selected pixels with a square or round brush.',detail:'Use Select or Deselect pixels and the selection mode to combine strokes.'},
 color:{key:'w',description:'Select matching colors across the artwork, including disconnected regions.'},
 wand:{key:'q',description:'Select a connected region of matching color.',detail:'Tolerance and merged-layer sampling control the matching region.'},
 move:{key:'v',description:'Move selected artwork, or the active layer when nothing is selected.'},
 zoom:{key:'z',description:'Click to zoom in or out at the pointer.',detail:'Choose the direction in tool options. Scroll also zooms.'},
 pan:{key:'a',description:'Drag to pan the artboard.',detail:'Hold `Space` or `Enter` to pan temporarily with any tool.'},
 text:{key:'t',description:'Click to draw text on the active layer.',detail:'Font, style, alignment, size, and antialiasing are set in tool options.'},
 paper:{description:'Fill the active layer inside the selection, or the whole artboard.'},
 pencil:{key:'p',description:'Draw with the selected color.',detail:'Hold `Shift` to connect a straight stroke from the previous endpoint.\nUse `Control + Wheel` to change the brush size.'},
 eraser:{key:'e',description:'Erase pixels with the configured brush.',detail:'Hold `Shift` to connect a straight stroke. Opacity controls the amount erased.\nUse `Control + Wheel` to change the brush size.'},
 bucket:{key:'b',description:'Fill connected colors, all matching colors, or the selection.',detail:'Optionally sample merged visible layers; output always changes the active layer.'},
 eyedropper:{key:'o',description:'Pick a pixel color from the active layer or merged artwork.',detail:'By default left-click sets the left color and right-click sets the right color.'},
 line:{key:'l',description:'Drag to draw a pixel line.',detail:'Hold `Shift` to snap every 22.5°, `Control` to mirror from the origin, or `Control + Shift` for both.\nHold `Alt` to move the origin.'},
 curve:{key:'j',description:'Click Bézier points; drag to adjust their tangents.',detail:'Double-click to finish. `Control` removes the latest point.\nHold `Alt` to move the origin. `Escape` cancels.'},
 'draw-rectangle':{key:'s',description:'Draw a rectangular outline.',detail:'Hold `Shift` for a square, `Control` to center, or `Control + Shift` for both.\nHold `Alt` to move the origin.'},
 'draw-ellipse':{key:'c',description:'Draw an elliptical outline.',detail:'Hold `Shift` for a circle, `Control` to center, or `Control + Shift` for both.\nHold `Alt` to move the origin.'},
 isometric:{description:'Click an origin, two base directions, and the height.',detail:'Hold `Shift` to snap base directions. `Control` edits the previous basis.\nHold `Alt` to move the origin. `Escape` cancels.'},
};
export function toolTitle(label:string){return label.replace(/\b[a-z]/g,letter=>letter.toUpperCase());}
/** Only backticks mark keys. Everything is rendered as escaped Vue text, never HTML. */
export function helpParts(text:string){return text.split('`').map((text,index)=>({text,key:index%2===1}));}
export function toolForKey(key:string):Tool|undefined{return (Object.keys(toolInfo) as Tool[]).find(id=>toolInfo[id].key===key.toLowerCase());}
export function toolTooltip(id:Tool,label:string){const info=toolInfo[id];return [toolTitle(label),info.key?`${info.key.toUpperCase()}: Left Button\nAlt+${info.key.toUpperCase()}: Right Button`:null,info.description,info.detail?.replaceAll('`','')].filter(Boolean).join('\n\n');}
