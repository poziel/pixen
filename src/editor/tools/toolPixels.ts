import {brush, type Point} from './raster';
export function selectionBrushMask(width:number,height:number,path:Point[],size:number,shape:'square'|'circle'){
 const data=new Uint8ClampedArray(width*height*4);
 brush(data,width,height,path,'#ffffffff',{size,shape,density:100,opacity:100});
 const mask=new Set<number>();for(let n=0;n<width*height;n++)if(data[n*4+3])mask.add(n);
 return mask;
}
export function pickedChannels(sample:string,current:string,mode:'rgba'|'rgb'|'alpha'){
 const sampled=sample.length===7?sample+'ff':sample,existing=current.length===7?current+'ff':current;
 return mode==='rgb'?sampled.slice(0,7)+existing.slice(7,9):mode==='alpha'?existing.slice(0,7)+sampled.slice(7,9):sampled;
}
