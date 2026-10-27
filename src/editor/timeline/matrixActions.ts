import type { MatrixCell } from './matrixSelection';

/** Copy from the far end first so adjacent selected sources are never overwritten early. */
export function directionalCopies(cells: readonly MatrixCell[], frames: readonly string[], direction: 1 | -1) {
  return [...cells].sort((a,b)=>direction*(frames.indexOf(b.frameId)-frames.indexOf(a.frameId))).flatMap(cell=>{
    const index=frames.indexOf(cell.frameId), destination=frames[index+direction];
    return index<0 || !destination ? [] : [{type:'copy_cells',layer_id:cell.layerId,source_frame_id:cell.frameId,frame_ids:[destination]}];
  });
}
export function moveHeader(order: readonly string[], source: string, target: string, after: boolean) {
  if(source===target || !order.includes(source) || !order.includes(target))return [...order];
  const rest=order.filter(id=>id!==source), index=rest.indexOf(target)+(after?1:0);
  return [...rest.slice(0,index),source,...rest.slice(index)];
}
