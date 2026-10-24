import themeText from '/assets/themes/pixloom.yaml?raw';

const documents=[];let active=null,next=0;
const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==';
function create(path,width=32,height=32){const id='document-'+(++next);documents.push({id,path,dirty:false,width,height,generation:1,revision:0,frames:path?.includes('animation')?Array.from({length:4},(_,i)=>({id:'frame-'+i,duration_ms:120})):[{id:'first',duration_ms:100}],layers:[{id:'layer',name:'Layer',visible:true,opacity:1}]});active=id;}
export async function call(command,args={}){
 window.testCalls??=[];window.testCalls.push({command,args});
 if(command==='snapshot'){const current=documents.find(doc=>doc.id===active);return structuredClone({managed:false,event_serial:0,review:null,review_stale:false,activity:null,inspection:null,active_document:active,documents:documents.map(({id,path,dirty})=>({id,path,dirty})),document:current?{...current,source_width:current.width,source_height:current.height,alternate_view:false,can_toggle_view:false}:null});}
 if(command==='desktop_ready')return;
 if(command==='theme_config')return {text:themeText,format:'yaml'};
 if(command==='open_document'){const path=args.path??'a.pixloom';const found=documents.find(doc=>doc.path===path);if(found)active=found.id;else create(path,path.includes('b.')?16:32);return true;}
 if(command==='create_document'){create(null,args.width,args.height);return;}
 if(command==='activate_document'){active=args.documentId;return;}
 if(command==='close_document'){const at=documents.findIndex(doc=>doc.id===args.documentId);documents.splice(at,1);if(active===args.documentId)active=documents.at(-1)?.id??null;return;}
 if(command==='save_document'){const doc=documents.find(doc=>doc.id===active);doc.dirty=false;doc.path??='saved.pixloom';return true;}
 if(command==='resize_document'){const doc=documents.find(doc=>doc.id===active);Object.assign(doc,{width:args.width,height:args.height,dirty:true,generation:doc.generation+1});return;}
 if(command==='render'){if(window.renderDelay)await new Promise(resolve=>setTimeout(resolve,window.renderDelay));const doc=documents.find(doc=>doc.id===active);if(args.documentId!==active)throw new Error('stale document');let image=png;if(args.request.frame_id.startsWith('frame-')){const canvas=document.createElement('canvas');canvas.width=doc.width;canvas.height=doc.height;const context=canvas.getContext('2d');context.fillStyle=['#cc4444','#44cc44','#4444cc','#ccaa44'][Number(args.request.frame_id.slice(6))%4];context.fillRect(0,0,canvas.width,canvas.height);image=canvas.toDataURL('image/png').split(',')[1];}return {width:doc.width,height:doc.height,png:image};}
}
