import type {ToolOptions} from './toolOptions';
import type {Point} from './raster';
export function drawToolText(ctx:CanvasRenderingContext2D,text:ToolOptions['text'],point:Point,color:string){
 const size=Math.max(1,Math.min(128,text.size));
 ctx.font=`${text.italic?'italic ':''}${text.bold?'bold ':''}${size}px "${text.font}", monospace`;
 // Generic family keywords must remain unquoted for browser fallback resolution.
 if(['monospace','serif','sans-serif'].includes(text.font))ctx.font=`${text.italic?'italic ':''}${text.bold?'bold ':''}${size}px ${text.font}`;
 ctx.textBaseline='top';ctx.fillStyle=color.slice(0,7);ctx.textAlign=text.align==='justify'?'left':text.align;
 const lines=text.content.split('\n'),width=Math.max(...lines.map(line=>ctx.measureText(line).width),0);
 lines.forEach((line,index)=>{
  const y=point.y+index*Math.ceil(size*1.2),words=line.trim().split(/\s+/);
  if(text.align==='justify'&&index<lines.length-1&&words.length>1){
   const gap=(width-words.reduce((sum,word)=>sum+ctx.measureText(word).width,0))/(words.length-1);let x=point.x;
   for(const word of words){ctx.fillText(word,x,y);x+=ctx.measureText(word).width+gap;}
  }else ctx.fillText(line,point.x,y);
 });
}
