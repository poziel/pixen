import {cellKey,type MatrixCell} from './matrixSelection';
export interface CopiedCels {width:number;height:number;cels:{row:number;column:number;pixels:number[][];opacity:number}[]}
export async function captureCels(cells:readonly MatrixCell[],rows:readonly string[],frames:readonly string[],width:number,height:number,read:(cell:MatrixCell)=>Promise<{pixels:number[];cel_opacity?:number}>) : Promise<CopiedCels> {
  if(!cells.length)throw new Error('Select cels to copy.');
  if(cells.length*width*height>16*1024*1024)throw new Error('This cel selection is too large to copy at once.');
  const top=Math.min(...cells.map(c=>rows.indexOf(c.layerId))),left=Math.min(...cells.map(c=>frames.indexOf(c.frameId)));
  const cels:CopiedCels['cels']=[];
  for(const cell of cells){
    const source=await read(cell);
    if(source.pixels.length!==width*height*4)throw new Error('Copied cel has an invalid pixel count.');
    const pixels:number[][]=[];for(let i=0;i<source.pixels.length;i+=4)pixels.push(source.pixels.slice(i,i+4));
    cels.push({row:rows.indexOf(cell.layerId)-top,column:frames.indexOf(cell.frameId)-left,pixels,opacity:source.cel_opacity??1});
  }
  return {width,height,cels};
}
export function planCelPaste(copied:CopiedCels,selected:readonly MatrixCell[],active:MatrixCell,rows:readonly string[],frames:readonly string[],width:number,height:number){
  if(copied.width!==width||copied.height!==height)throw new Error('Copied cel size differs from this artwork.');
  const matchingSelection=copied.cels.length>1&&selected.length>1;
  const top=matchingSelection?Math.min(...selected.map(cell=>rows.indexOf(cell.layerId))):rows.indexOf(active.layerId);
  const left=matchingSelection?Math.min(...selected.map(cell=>frames.indexOf(cell.frameId))):frames.indexOf(active.frameId);
  const targets=copied.cels.length===1
    ? (selected.length?selected:[active]).map(cell=>({cell,source:copied.cels[0]!}))
    : copied.cels.map(source=>({cell:{layerId:rows[top+source.row]!,frameId:frames[left+source.column]!},source}));
  if(top<0||left<0||targets.some(({cell})=>!cell.layerId||!cell.frameId))throw new Error('The copied cels do not fit here. Select an earlier frame or higher layer.');
  if(matchingSelection){const keys=new Set(selected.map(cellKey));if(targets.length!==keys.size||targets.some(({cell})=>!keys.has(cellKey(cell))))throw new Error('Select one destination cel or a selection matching the copied layout.');}
  const groups=new Map<string,{type:string;layer_id:string;frame_ids:string[];width:number;height:number;pixels:number[][];opacity:number}>();
  for(const {cell,source} of targets){
    const key=JSON.stringify([cell.layerId,source.row,source.column]);let op=groups.get(key);
    if(!op){op={type:'paste_cells',layer_id:cell.layerId,frame_ids:[],width,height,pixels:source.pixels,opacity:source.opacity};groups.set(key,op);}op.frame_ids.push(cell.frameId);
  }
  return [...groups.values()];
}
