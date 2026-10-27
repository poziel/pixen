export function durationOperations(ids:readonly string[],duration:number){
 if(!Number.isFinite(duration)||duration<=0)throw new Error('Frame duration must be greater than zero milliseconds.');
 return [...new Set(ids)].map(frame_id=>({type:'set_frame_duration',frame_id,duration_ms:duration}));
}
export function previousFrameCopies(layers:readonly string[],frames:readonly string[],destination:string){
 const index=frames.indexOf(destination);if(index<1)return [];
 return layers.map(layer_id=>({type:'copy_cells',layer_id,source_frame_id:frames[index-1]!,frame_ids:[destination]}));
}
