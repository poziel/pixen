import type {RenderRequest} from '../../services/backend';
/** Same display-only tint/comparison rules as core/document/inspection.rs. */
export function inspectionPreview(current:Uint8ClampedArray,mode:RenderRequest['mode'],previous?:Uint8ClampedArray,next?:Uint8ClampedArray){
 if(mode==='composite'||!mode)return current;
 if(mode==='frame_comparison'){
  const other=previous??current,out=current.slice();
  for(let i=0;i<out.length;i+=4){if(current[i]!==other[i]||current[i+1]!==other[i+1]||current[i+2]!==other[i+2]||current[i+3]!==other[i+3]){out[i]=255;out[i+1]=64;out[i+2]=180;out[i+3]=255;}else out[i+3]=Math.floor(current[i+3]*60/255);}return out;
 }
 const out=new Uint8ClampedArray(current.length);
 function over(i:number,r:number,g:number,b:number,sa:number){const da=out[i+3],a=sa*255+da*(255-sa);if(!a){out.fill(0,i,i+4);return;}for(let k=0;k<3;k++){const color=k===0?r:k===1?g:b;out[i+k]=Math.floor((color*sa*255+out[i+k]*da*(255-sa)+Math.floor(a/2))/a);}out[i+3]=Math.floor((a+127)/255);}
 for(const [ghost,tint] of [[previous,[255,80,80]],[next,[60,210,255]]] as const){if(ghost)for(let i=0;i<out.length;i+=4)over(i,tint[0],tint[1],tint[2],Math.floor(ghost[i+3]*90/255));}
 for(let i=0;i<out.length;i+=4)over(i,current[i],current[i+1],current[i+2],current[i+3]);return out;
}
