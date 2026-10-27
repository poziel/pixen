import type {FrameMenuItem} from '@frame/core';
export const previewFps=[0.5,1,3,5,8,15,20] as const;
export function previewSpeedItems(fps:number):FrameMenuItem[]{return [{id:'0',label:'Use frame timings',checked:fps===0},...previewFps.map(value=>({id:String(value),label:`${value} FPS`,checked:fps===value}))];}
