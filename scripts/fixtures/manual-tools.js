import themeText from '/assets/themes/pixloom.yaml?raw';
let opened=false,generation=1,revision=0;
const width=16,height=16,frames=[{id:'first',duration_ms:100}],layers=[{id:'layer',name:'Layer',visible:true,opacity:1}];
let pixels=new Uint8ClampedArray(width*height*4),undo=[],redo=[];
window.toolPixels=()=>Array.from(pixels);
window.testCalls=[];
export async function call(command,args={}){
  window.testCalls.push({command,args});
  if(command==='snapshot')return {managed:false,event_serial:0,active_document:opened?'drawing':null,documents:opened?[{id:'drawing',path:'drawing.pixloom',dirty:revision>0}]:[],document:opened?{width,height,source_width:width,source_height:height,generation,revision,frames,layers,path:'drawing.pixloom',dirty:revision>0,alternate_view:false}:null};
  if(command==='open_document'){opened=true;return true;}
  if(command==='theme_config')return {text:themeText,format:'yaml'};
  if(command==='read_cel'){await new Promise(r=>setTimeout(r,35));return {pixels:Array.from(pixels),opacity:1};}
  if(command==='paint_pixels'){
    if(args.generation!==generation)throw new Error('stale stroke');
    undo.push(pixels.slice());redo=[];
    for(const p of args.pixels)pixels.set([1,3,5,7].map(i=>parseInt(p.color.slice(i,i+2),16)),(p.y*width+p.x)*4);
    generation++;revision++;return;
  }
  if(command==='edit_history'){
    const source=args.redo?redo:undo,target=args.redo?undo:redo;
    if(!source.length)throw new Error('no history');
    target.push(pixels.slice());pixels=source.pop();generation++;revision++;return;
  }
  if(command==='render'){
    const c=document.createElement('canvas');c.width=width;c.height=height;if(!args.request.hidden_layers?.includes('layer'))c.getContext('2d').putImageData(new ImageData(pixels.slice(),width,height),0,0);
    return {width,height,png:c.toDataURL().split(',')[1]};
  }
}

