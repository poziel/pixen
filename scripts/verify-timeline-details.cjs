// UI sketch coverage. Native cell semantics are covered in tests/authoring/editing.rs.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const fixture = `
import themeText from '/assets/themes/pixen.yaml?raw';
let opened=false,generation=1,revision=0;
const frames=Array.from({length:10},(_,i)=>({id:'frame-'+i,duration_ms:100}));
const layers=['Background','Body','Face','Highlights'].map((name,i)=>({id:'layer-'+i,name,visible:true,opacity:1,locked:false,cells:frames.map((f,n)=>({cel:i===3&&n>5?null:'cel-'+i+'-'+n,link:null}))}));
function png(request) {
 const c=document.createElement('canvas');c.width=16;c.height=16;const ctx=c.getContext('2d');
 const n=Math.max(0,frames.findIndex(f=>f.id===request.frame_id)), lift=n%4===1?1:0;
 for(const l of layers) {
  const i=Number(l.id.replace('layer-',''));
  if(request.layer_id&&request.layer_id!==l.id||request.hidden_layers?.includes(l.id)||!l.cells[n]?.cel)continue;
  if(i===0){ctx.fillStyle='#223842';ctx.fillRect(1,13,14,2);ctx.fillStyle='#36584c';ctx.fillRect(3,12,10,1);}
  if(i===1){ctx.fillStyle='#f1aa58';ctx.fillRect(4,5-lift,8,7);ctx.fillRect(3,7-lift,10,3);ctx.fillStyle='#bf694d';ctx.fillRect(4,11-lift,8,1);ctx.fillRect(5,12-lift,2,1);ctx.fillRect(9,12-lift,2,1);}
  if(i===2){ctx.fillStyle='#342e40';ctx.fillRect(5,7-lift,1,2);ctx.fillRect(10,7-lift,1,2);ctx.fillRect(7,10-lift,2,1);ctx.fillStyle='#ee706b';ctx.fillRect(4,9-lift,2,1);ctx.fillRect(10,9-lift,2,1);}
  if(i===3){ctx.fillStyle='#ffe3a0';ctx.fillRect(5,5-lift,3,1);ctx.fillRect(4,6-lift,1,2);}
 }
 return {width:16,height:16,png:c.toDataURL().split(',')[1]};
}
window.matrixCalls=[];
window.matrixState=()=>structuredClone({frames,layers});
window.matrixSetLocked=(id,value)=>{layers.find(layer=>layer.id===id).locked=value;generation++;};
export async function call(command,args={}) {
 window.matrixCalls.push({command,args});
 if(command==='theme_config')return {text:themeText,format:'yaml'};
 if(command==='open_document'){opened=true;return true;}
 if(command==='snapshot')return structuredClone({managed:false,event_serial:0,active_document:opened?'sketch':null,documents:opened?[{id:'sketch',path:'ember-walk.pixloom',dirty:revision>0}]:[],document:opened?{width:16,height:16,source_width:16,source_height:16,generation,revision,frames,layers,path:'ember-walk.pixloom',dirty:revision>0,alternate_view:false}:null});
 if(command==='render')return png(args.request);
 if(command==='read_cel'){
  const entry=layers.find(l=>l.id===args.layerId).cells[frames.findIndex(f=>f.id===args.frameId)];
  const color=entry.cel?[80+(entry.cel.length%100),40,120,255]:[0,0,0,0];
  const pixels=entry.cel&&entry.rgba?entry.rgba.slice():Array.from({length:256},()=>color).flat();window.matrixLastRead=pixels;
  return {pixels,opacity:1,cel_opacity:entry.opacity??1};
 }
 if(command==='edit_structure') {
  assertGeneration(args.generation);
  for(const op of [].concat(args.operation)) {
   if(op.type==='add_layer')layers.splice(op.index,0,{id:op.layer_id,name:op.name,visible:true,opacity:1,cells:frames.map(()=>({cel:null,link:null}))});
   if(op.type==='add_frame'||op.type==='duplicate_frame'){
    const source=frames.findIndex(f=>f.id===op.frame_id);
    frames.splice(op.index,0,{id:op.new_frame_id??op.frame_id,duration_ms:op.duration_ms??100});
    for(const l of layers)l.cells.splice(op.index,0,op.type==='duplicate_frame'?{...l.cells[source],link:null}:{cel:null,link:null});
   }
   if(op.type==='rename_frame')frames.find(f=>f.id===op.frame_id).name=op.name;
   if(op.type==='set_frame_duration')frames.find(f=>f.id===op.frame_id).duration_ms=op.duration_ms;
   if(op.type==='paste_cells'){const layer=layers.find(l=>l.id===op.layer_id);for(const id of op.frame_ids){const cell=layer.cells[frames.findIndex(f=>f.id===id)];cell.cel='pasted-'+revision+'-'+id;cell.rgba=op.pixels.flat();cell.opacity=op.opacity;cell.link=null;}}
   if(op.type==='rename_layer')layers.find(l=>l.id===op.layer_id).name=op.name;
   if(op.type==='remove_layer')layers.splice(layers.findIndex(l=>l.id===op.layer_id),1);
   if(op.type==='remove_frame'){const index=frames.findIndex(f=>f.id===op.frame_id);frames.splice(index,1);for(const layer of layers)layer.cells.splice(index,1);}
   if(op.type==='reorder_layer'){const index=layers.findIndex(l=>l.id===op.layer_id);layers.splice(op.index,0,layers.splice(index,1)[0]);}
   if(op.type==='reorder_frame'){const index=frames.findIndex(f=>f.id===op.frame_id);frames.splice(op.index,0,frames.splice(index,1)[0]);for(const layer of layers)layer.cells.splice(op.index,0,layer.cells.splice(index,1)[0]);}
   if(op.type==='link_cells'||op.type==='copy_cells'||op.type==='unlink_cells'||op.type==='clear_cells'){
    const l=layers.find(l=>l.id===op.layer_id),source=l.cells[frames.findIndex(f=>f.id===op.source_frame_id)];
    for(const id of op.frame_ids){const cell=l.cells[frames.findIndex(f=>f.id===id)];if(op.type==='clear_cells')cell.cel=null;else if(op.type!=='unlink_cells')cell.cel=source.cel;cell.link=op.type==='link_cells'?op.link_id:null;}
   }
  }
  generation++;revision++;
 }
}
function assertGeneration(value){if(value!==generation)throw new Error('stale edit');}
`;
(async () => {
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try {
  const page=await browser.newPage({viewport:{width:1440,height:960},deviceScaleFactor:1}), errors=[];
  page.on('pageerror',e=>{ errors.push(e.message); console.error('Browser error:', e.message); });
  await page.route('**/src/services/backend.ts*',r=>r.fulfill({contentType:'text/javascript',body:fixture}));
  await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1459', { waitUntil: 'commit' });
  const toggle=page.getByRole('button',{name:'Toggle timeline matrix',exact:true});
  assert(await toggle.isDisabled());
  await page.locator('.pane-welcome-actions').getByRole('button',{name:/Open artwork/}).click();
  await page.getByRole('button',{name:'Timeline',exact:true}).click();
  const initialIcon=await toggle.locator('svg').innerHTML();
  await toggle.click();
  const matrix=page.locator('.timeline-matrix:visible');await matrix.waitFor();

  await page.locator('.status-right [data-status-id="pixen.fps"]').getByText('Frame timings',{exact:true}).waitFor();
  assert.equal(await page.locator('.status-right [data-bar-separator="pixen.matrix-selection.separator"]').count(),1);
  const header=matrix.locator('.matrix-frame').first();await header.hover();
  assert.notEqual(await header.evaluate(el=>getComputedStyle(el).backgroundColor),'rgba(0, 0, 0, 0)');
  const bounds=await header.boundingBox(),timing=await header.locator('small').boundingBox();assert(timing.y>=bounds.y&&timing.y+timing.height<=bounds.y+bounds.height,'duration is inside highlighted header');
  const cell=(row,col)=>matrix.locator('[data-cell="'+row+'-'+col+'"] button');
  await cell(2,0).click();await cell(2,4).click({modifiers:['Shift']});
  await page.locator('.status-right [data-status-id="pixen.matrix-selection"]').getByText('Body · 5 cels',{exact:true}).waitFor();
  await cell(1,4).click({modifiers:['Shift']});
  await page.locator('.status-right [data-status-id="pixen.matrix-selection"]').getByText('2 layers · 10 cels',{exact:true}).waitFor();
  for(const kind of ['frame','layer']){
    const headers=matrix.locator('[data-structure-kind='+kind+']');const a=await headers.first().boundingBox(),b=await headers.nth(2).boundingBox();
    await page.mouse.move(a.x+a.width*.6,a.y+a.height*.5);await page.mouse.down();await page.mouse.move(b.x+b.width*.6,b.y+b.height*.5,{steps:10});
    const ghost=page.locator('.frame-drag-preview .structure-drag-preview');await ghost.waitFor();assert(await ghost.locator('canvas,img').count()>0,'drag ghost has a thumbnail');assert((await ghost.innerText()).length>0);await page.mouse.up();await ghost.waitFor({state:'hidden'});
  }
  await toggle.click();
  const row=page.locator('.preview-row:visible').first();await row.locator('small').dblclick();
  const duration=page.getByRole('textbox',{name:/duration \(ms\)/}).first();await duration.fill('175');await duration.press('Enter');await row.locator('small').getByText('175 ms',{exact:true}).waitFor();
  assert.deepEqual(errors,[]);console.log('PASS: default timings, right-side layer-first selection, shared matrix frame/layer drag ghosts, sidebar editable duration.');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
