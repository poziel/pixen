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
window.matrixSetLocked=(id,value)=>{layers.find(layer=>layer.id===id).locked=value;generation++;};
export async function call(command,args={}) {
 window.matrixCalls.push({command,args});
 if(command==='theme_config')return {text:themeText,format:'yaml'};
 if(command==='open_document'){opened=true;return true;}
 if(command==='snapshot')return structuredClone({managed:false,event_serial:0,active_document:opened?'sketch':null,documents:opened?[{id:'sketch',path:'ember-walk.pixloom',dirty:revision>0}]:[],document:opened?{width:16,height:16,source_width:16,source_height:16,generation,revision,frames,layers,path:'ember-walk.pixloom',dirty:revision>0,alternate_view:false}:null});
 if(command==='render')return png(args.request);
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

  const list=page.locator('.structure-list:visible');
  await page.getByRole('button',{name:'Layers',exact:true}).click();
  await list.locator('.preview-row strong').first().dblclick();
  await list.getByRole('textbox').fill('Layer click-away saved');
  await page.getByRole('button',{name:'Timeline',exact:true}).click();
  await page.waitForFunction(()=>window.matrixCalls.some(call=>call.command==='edit_structure' && call.args.operation.type==='rename_layer' && call.args.operation.name==='Layer click-away saved'));
  await page.getByRole('button',{name:'Layers',exact:true}).click();
  await list.getByText('Layer click-away saved',{exact:true}).dblclick();await list.getByRole('textbox').fill('Layer Tab saved');await list.getByRole('textbox').press('Tab');
  await list.getByText('Layer Tab saved',{exact:true}).waitFor();
  await list.getByText('Layer Tab saved',{exact:true}).dblclick();await list.getByRole('textbox').fill('Discarded layer');await list.getByRole('textbox').press('Escape');
  await page.getByRole('button',{name:'Timeline',exact:true}).click();
  await list.locator('.preview-row strong').first().dblclick();await list.getByRole('textbox').fill('Frame click-away saved');
  await page.getByRole('button',{name:'Layers',exact:true}).click();
  await page.waitForFunction(()=>window.matrixCalls.some(call=>call.command==='edit_structure' && call.args.operation.type==='rename_frame' && call.args.operation.name==='Frame click-away saved'));
  await toggle.click();const matrix=page.locator('.timeline-matrix:visible');await matrix.waitFor();
  const layer=matrix.locator('[role=rowheader]').first();
  await layer.getByRole('button').last().dblclick();await layer.getByRole('textbox').fill('Matrix layer saved');await layer.getByRole('textbox').press('Tab');
  await layer.getByRole('button',{name:'Matrix layer saved',exact:true}).waitFor();
  const header=matrix.locator('[role=columnheader]').nth(1);
  await header.getByRole('button').dblclick();await header.getByRole('textbox').fill('Matrix frame saved');
  await page.getByRole('button',{name:'Tools',exact:true}).click();
  await header.getByRole('button',{name:'Matrix frame saved',exact:true}).waitFor();
  await header.getByRole('button').dblclick();await header.getByRole('textbox').fill('Discarded frame');await header.getByRole('textbox').press('Escape');
  await toggle.click();
  const names=await page.evaluate(()=>window.matrixCalls.filter(call=>call.command==='edit_structure' && call.args.operation.type?.startsWith('rename_')).map(call=>call.args.operation.name));
  assert(!names.includes('Discarded layer'));assert(!names.includes('Discarded frame'));
  assert.deepEqual(errors,[]);console.log('PASS: layer and frame view-switch commits, Tab commits, matrix layer/frame rename, Escape cancellation, and no accidental save after cancellation.');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exit(1)});
