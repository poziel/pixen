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
  assert.notEqual(await toggle.locator('svg').innerHTML(),initialIcon,'icon changes with timeline layout');
  assert.equal(await toggle.getAttribute('aria-expanded'),'true');
  assert.equal(await page.getByRole('button',{name:'Layers',exact:true}).count(),1);
  assert.equal(await page.getByRole('button',{name:'Timeline',exact:true}).count(),1);
  assert.equal(await page.locator('.panel-view-bar:visible').count(),0);
  assert.equal(await page.locator('#bottom-panel .panel-header').count(),0,'only the application toolbar remains');
  assert.equal(await matrix.locator('.matrix-title').count(),0,'matrix has no redundant title');
  assert.equal(await matrix.locator('.playback-full').getByRole('button',{name:'Preview speed',exact:true}).count(),0,'preview speed is separate from transport controls');
  const cell=(row,col)=>matrix.locator('[data-cell="'+row+'-'+col+'"] button');
  assert(await matrix.getByRole('button',{name:'Cut selected cels',exact:true}).locator('svg path').count()>0,'Cut has a real local icon');
  await matrix.locator('.matrix-frame').first().locator('small').dblclick();
  assert.equal(await page.getByRole('dialog').count(),0,'double-click edits the duration inline');
  const inlineDuration=matrix.getByRole('textbox',{name:'Frame 1 duration (ms)',exact:true});
  await inlineDuration.fill('55');await inlineDuration.press('Enter');
  await page.waitForTimeout(160);assert.equal(await matrix.locator('.matrix-frame').first().locator('small').innerText(),'55 ms');
  await matrix.locator('.matrix-frame').first().locator('small').dblclick();await inlineDuration.fill('900');await inlineDuration.press('Escape');
  assert.equal(await matrix.locator('.matrix-frame').first().locator('small').innerText(),'55 ms');
  await matrix.locator('.matrix-frame').first().locator('small').dblclick();await inlineDuration.fill('60');await cell(2,0).click();await page.waitForTimeout(160);
  assert.equal(await matrix.locator('.matrix-frame').first().locator('small').innerText(),'60 ms','leaving the inline field saves');
  await matrix.locator('.matrix-frame').first().locator('small').dblclick();await inlineDuration.fill('0');await inlineDuration.press('Enter');
  assert.equal(await matrix.locator('.matrix-frame').first().locator('small').innerText(),'60 ms','invalid values preserve timing');
  await cell(2,0).click();await cell(2,1).click({modifiers:['Shift']});
  await matrix.locator('.matrix-frame').first().click({button:'right'});
  await page.getByRole('menu',{name:'Timeline context menu',exact:true}).getByRole('menuitem',{name:'Duration of selected frames…',exact:true}).click();
  const durationDialog=page.getByRole('dialog',{name:'Frame durations',exact:true});
  await durationDialog.getByRole('spinbutton',{name:'Duration (ms)',exact:true}).fill('80');await durationDialog.getByRole('button',{name:'Apply',exact:true}).click();await page.waitForTimeout(160);
  assert.deepEqual(await matrix.locator('.matrix-frame small').evaluateAll(items=>items.slice(0,2).map(item=>item.textContent)),['80 ms','80 ms']);
  await matrix.getByRole('button',{name:'Preview speed',exact:true}).click();
  await page.getByRole('menuitemcheckbox',{name:'8 FPS',exact:true}).click();
  await page.locator('[data-status-id="pixen.fps"]').getByText('8 FPS',{exact:true}).waitFor();
  await matrix.getByRole('button',{name:'Preview speed',exact:true}).click();
  await page.getByRole('menuitemcheckbox',{name:'Use frame timings',exact:true}).click();
  await page.locator('[data-status-id="pixen.fps"]').getByText('Frame timings',{exact:true}).waitFor();
  await matrix.getByRole('button',{name:'Select frame cels',exact:true}).click();assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),4);
  await cell(2,1).click();await cell(2,4).click({modifiers:['Shift']});
  assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),4);
  await page.waitForTimeout(1100);
  assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),4,'polling retains range');
  await matrix.getByRole('button',{name:'Link cels',exact:true}).click();
  await page.getByRole('alertdialog').getByRole('button',{name:'Link cels',exact:true}).click();
  await page.waitForTimeout(150);
  assert.equal(await matrix.getByRole('button',{name:/Body, frame .*linked/}).count(),4);
  await cell(2,4).press('ArrowLeft');
  assert(await cell(2,3).evaluate(el=>el===document.activeElement));
  await matrix.getByRole('button',{name:'Unlink selected cels',exact:true}).click();await page.waitForTimeout(150);
  assert.equal(await matrix.getByRole('button',{name:/Body, frame .*linked/}).count(),3);
  // Vertical and diagonal Shift ranges preserve the original anchor across layers.
  await cell(0,2).click();await cell(3,2).click({modifiers:['Shift']});
  assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),4);
  await cell(0,1).click();await cell(2,3).click({modifiers:['Shift']});
  assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),9);
  await matrix.getByRole('button',{name:'Link cels',exact:true}).click();
  await page.getByRole('alertdialog').getByRole('button',{name:'Link cels',exact:true}).click();await page.waitForTimeout(150);
  const links=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.equal(links.length,3);assert.equal(new Set(links.map(op=>op.link_id)).size,3,'link groups remain separate for each layer');
  // Sparse Ctrl selection must not include any intervening cells.
  await cell(1,1).click();
  for(const [row,col] of [[1,6],[3,8],[2,9]])await cell(row,col).click({modifiers:['Control']});
  assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),4);
  await page.evaluate(()=>window.matrixSetLocked('layer-1',true));await page.waitForTimeout(400);
  assert(await matrix.getByRole('button',{name:'Clear selected cels',exact:true}).isDisabled(),'any selected locked layer blocks the batch');
  await page.evaluate(()=>window.matrixSetLocked('layer-1',false));await page.waitForTimeout(400);
  await cell(2,9).press('Delete');await page.waitForTimeout(150);
  const clear=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.equal(clear.length,3);assert.equal(clear.reduce((n,op)=>n+op.frame_ids.length,0),4);
  for(const [row,col] of [[1,1],[1,6],[3,8],[2,9]])assert((await cell(row,col).getAttribute('aria-label')).includes('empty'));
  assert(!(await cell(2,0).getAttribute('aria-label')).includes('empty'),'unselected artwork survives');
  // Real pointer drags select horizontally and diagonally, with no native artwork move.
  async function drag(from,to,modifiers=[]) {
   await cell(...from).scrollIntoViewIfNeeded();await cell(...to).scrollIntoViewIfNeeded();
   const a=await cell(...from).boundingBox(),b=await cell(...to).boundingBox();
   for(const modifier of modifiers)await page.keyboard.down(modifier);
   await page.mouse.move(a.x+a.width/2,a.y+a.height/2);await page.mouse.down();
   await page.mouse.move(b.x+b.width/2,b.y+b.height/2,{steps:12});await page.mouse.up();
   for(const modifier of modifiers)await page.keyboard.up(modifier);
  }
  await drag([1,1],[1,4]);assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),4);
  await drag([0,1],[2,3]);assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),9);
  await cell(3,9).click();await drag([0,1],[1,2],['Control']);
  assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),5);
  await cell(1,2).press('Shift+ArrowDown');
  assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),6);
  await matrix.getByRole('button',{name:'Empty frame',exact:true}).click();await page.waitForTimeout(150);
  assert.equal(await matrix.getByRole('columnheader').count(),12);
  await cell(2,2).click();const beforeNew=await page.evaluate(()=>window.matrixState());
  await matrix.getByRole('button',{name:'New frame',exact:true}).click();await page.waitForTimeout(150);
  const created=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);assert.equal(created.type,'duplicate_frame','New frame copies all layers through native duplication');
  const afterNew=await page.evaluate(()=>window.matrixState());assert.deepEqual(afterNew.layers.map(layer=>layer.cells[3].cel),beforeNew.layers.map(layer=>layer.cells[2].cel));assert(afterNew.layers.every(layer=>!layer.cells[3].link));
  assert.equal(await matrix.getByRole('columnheader').count(),13);
  await matrix.getByRole('button',{name:'New layer',exact:true}).click();await page.waitForTimeout(150);
  assert.equal(await matrix.getByRole('rowheader').count(),5);
  await toggle.click();
  assert.equal(await page.locator('.timeline-matrix:visible').count(),0);
  await page.getByRole('button',{name:'Timeline',exact:true}).waitFor();
  await toggle.click();await matrix.waitFor();
  await cell(1,0).click();await cell(2,2).click({modifiers:['Shift']});
  await page.waitForTimeout(250);
  assert.equal(await matrix.locator('.matrix-footer').count(),0);
  assert((await page.locator('[data-status-id="pixen.matrix-selection"]').innerText()).includes('6 cels'));
  assert.equal(await matrix.locator('[role=gridcell][aria-selected=true]').count(),6);
  // Editing names uses the same Enter/Escape behavior as the sidebar.
  const bodyHeader=()=>matrix.locator('[data-structure-kind="layer"][data-structure-id="layer-1"]');
  await bodyHeader().getByRole('button',{name:'Body',exact:true}).dblclick();
  await matrix.getByRole('textbox',{name:'Rename Body',exact:true}).fill('Renamed body');
  await matrix.getByRole('textbox').press('Enter');
  await bodyHeader().getByRole('button',{name:'Renamed body',exact:true}).waitFor();
  await bodyHeader().getByRole('button',{name:'Renamed body',exact:true}).press('F2');
  await matrix.getByRole('textbox').fill('Discard this');await matrix.getByRole('textbox').press('Escape');
  await bodyHeader().getByRole('button',{name:'Renamed body',exact:true}).dblclick();
  await matrix.getByRole('textbox').fill('Body');await matrix.getByRole('textbox').press('Enter');
  await bodyHeader().getByRole('button',{name:'Body',exact:true}).waitFor();
  // Copy is a snapshot, not a reference to the source cel. Paste can cross layers and frames.
  await cell(2,0).click();await cell(2,0).press('Control+c');
  await matrix.getByRole('button',{name:'Paste into selected cels',exact:true}).waitFor();
  await page.waitForFunction(()=>window.matrixLastRead?.length===1024);
  await page.waitForFunction(()=>document.body.innerText.includes('Copied 1 cel.'));
  const clipboardPixels=await page.evaluate(()=>window.matrixLastRead.slice());
  await cell(2,0).press('Delete');await page.waitForTimeout(160);
  await cell(1,5).click();await cell(1,5).press('Control+v');await page.waitForTimeout(180);
  let pasted=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.equal(pasted[0].type,'paste_cells');assert.equal(pasted[0].layer_id,'layer-2');assert.deepEqual(pasted[0].pixels.flat(),clipboardPixels);
  await cell(1,0).click();await cell(2,1).click({modifiers:['Shift']});
  await matrix.getByRole('button',{name:'Paste into selected cels',exact:true}).click();await page.waitForTimeout(180);
  pasted=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.equal(pasted.reduce((count,op)=>count+op.frame_ids.length,0),4,'one copied cel fills selected destinations across layers');
  const context=page.getByRole('menu',{name:'Timeline context menu',exact:true});
  async function contextAction(locator,name){await locator.click({button:'right'});await context.getByRole('menuitem',{name:new RegExp('^'+name+'(?:$| )')}).click();await page.waitForTimeout(160);}
  const countBefore=await matrix.locator('.matrix-frame').count();const timingBefore=await matrix.locator('.matrix-frame').nth(1).locator('small').innerText();
  await contextAction(matrix.locator('.matrix-frame').nth(1),'Copy previous frame here');
  const filled=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.equal(filled.length,5);assert(filled.every(op=>op.type==='copy_cells'&&op.source_frame_id==='frame-0'));
  assert.equal(await matrix.locator('.matrix-frame').count(),countBefore);assert.equal(await matrix.locator('.matrix-frame').nth(1).locator('small').innerText(),timingBefore);
  await matrix.getByRole('button',{name:'Copy selected cels',exact:true}).click();
  await page.waitForFunction(()=>document.body.innerText.includes('Copied 4 cels.'));
  await contextAction(cell(2,3),'Paste into selected cels');
  pasted=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.equal(pasted.length,4,'multi-cel clipboard retains a two-layer/two-frame layout');
  assert.equal(new Set(pasted.map(op=>op.layer_id)).size,2);
  await cell(2,0).click();await cell(2,0).press('Control+x');await page.waitForTimeout(250);
  let cut=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.equal(cut[0].type,'clear_cells');assert((await cell(2,0).getAttribute('aria-label')).includes('empty'));
  await cell(1,3).click();await cell(1,3).press('Control+v');await page.waitForTimeout(180);
  cut=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);assert.equal(cut[0].type,'paste_cells');assert.equal(cut[0].layer_id,'layer-2');
  await cell(2,0).click();await contextAction(cell(2,0),'Copy to the Right');
  let copied=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.equal(copied.length,1);assert.equal(copied[0].layer_id,'layer-1');assert.equal(copied[0].source_frame_id,'frame-0');assert.deepEqual(copied[0].frame_ids,['frame-1']);
  await cell(2,1).click();await contextAction(cell(2,1),'Copy to the Left');
  copied=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.deepEqual(copied[0].frame_ids,['frame-0']);
  await contextAction(matrix.locator('.matrix-frame').first(),'Copy to the Right');
  copied=await page.evaluate(()=>window.matrixCalls.filter(c=>c.command==='edit_structure').at(-1).args.operation);
  assert.equal(copied.length,5,'frame header copies every layer in the column');
  // Detach one linked cel while keeping the remaining selection and partners intact.
  await cell(2,0).click();await cell(2,2).click({modifiers:['Shift']});
  await contextAction(cell(2,0),'Link selected cels');
  await page.getByRole('alertdialog').getByRole('button',{name:'Link cels',exact:true}).click();await page.waitForTimeout(160);
  await contextAction(cell(2,1),'Unlink this cel');
  assert.equal(await cell(2,1).locator('.matrix-link').count(),0);
  assert.equal(await cell(2,0).locator('.matrix-link').count(),1);assert.equal(await cell(2,2).locator('.matrix-link').count(),1);
  assert.equal(await matrix.locator('[aria-selected=true][role=gridcell]').count(),3);
  await contextAction(cell(2,2),'Unlink selected cels');
  for(const index of [0,1,2])assert.equal(await cell(2,index).locator('.matrix-link').count(),0);
  // Header drags move complete structures rather than selected cell contents.
  const layerIds=()=>matrix.locator('[data-structure-kind=layer]').evaluateAll(items=>items.map(item=>item.dataset.structureId));
  const frameIds=()=>matrix.locator('[data-structure-kind=frame]').evaluateAll(items=>items.map(item=>item.dataset.structureId));
  async function moveHeader(kind,source,target,after){
    const a=await matrix.locator('[data-structure-kind="'+kind+'"][data-structure-id="'+source+'"]').boundingBox();
    const b=await matrix.locator('[data-structure-kind="'+kind+'"][data-structure-id="'+target+'"]').boundingBox();
    await page.mouse.move(a.x+a.width/2,a.y+a.height/2);await page.mouse.down();
    await page.mouse.move(kind==='layer'?b.x+b.width/2:after?b.x+b.width-2:b.x+2,kind==='layer'?after?b.y+b.height-2:b.y+2:b.y+b.height/2,{steps:12});
    assert.equal(await matrix.locator(kind==='layer'?'.row-insert-before,.row-insert-after':'.insert-before,.insert-after').count(),1);
    await page.mouse.up();await page.waitForTimeout(180);
  }
  const originalLayers=await layerIds();
  await moveHeader('layer',originalLayers[2],originalLayers[0],false);
  assert.deepEqual(await layerIds(),[originalLayers[2],originalLayers[0],originalLayers[1],...originalLayers.slice(3)]);
  await moveHeader('layer',originalLayers[2],originalLayers[1],true);assert.deepEqual(await layerIds(),originalLayers);
  const selectionIds=()=>matrix.locator('[role=gridcell][aria-selected=true]').evaluateAll(items=>items.map(item=>{
    const frame=item.closest('[role=grid]').querySelectorAll('[data-structure-kind=frame]')[Number(item.dataset.column)].dataset.structureId;
    const layer=item.closest('[role=row]').querySelector('[data-structure-kind=layer]').dataset.structureId;return layer+':'+frame;
  }).sort());
  await cell(1,0).click();await cell(2,2).click({modifiers:['Shift']});const priorSelection=await selectionIds();
  const originalFrames=await frameIds();
  await moveHeader('frame',originalFrames[0],originalFrames[3],true);
  assert.deepEqual(await frameIds(),[...originalFrames.slice(1,4),originalFrames[0],...originalFrames.slice(4)]);
  assert.deepEqual(await selectionIds(),priorSelection,'reordering preserves selected stable cell IDs');
  await moveHeader('frame',originalFrames[0],originalFrames[1],false);assert.deepEqual(await frameIds(),originalFrames);
  await matrix.locator('[data-structure-kind=frame][data-structure-id="'+originalFrames[0]+'"]').getByRole('button').press('Alt+ArrowRight');await page.waitForTimeout(180);
  assert.deepEqual((await frameIds()).slice(0,2),[originalFrames[1],originalFrames[0]]);
  await matrix.locator('[data-structure-kind=frame][data-structure-id="'+originalFrames[0]+'"]').getByRole('button').press('Alt+ArrowLeft');await page.waitForTimeout(180);assert.deepEqual(await frameIds(),originalFrames);
  // One confirmation dialog owns each destructive action, even with sidebar views mounted.
  const extra=originalLayers.find(id=>!id.startsWith('layer-0')&&!['layer-1','layer-2','layer-3'].includes(id));
  await contextAction(matrix.locator('[data-structure-id="'+extra+'"]'),'Delete Layer');
  assert.equal(await page.getByRole('alertdialog').count(),1);await page.getByRole('alertdialog').getByRole('button',{name:'Cancel',exact:true}).click();
  assert.equal((await layerIds()).length,5);
  await contextAction(matrix.locator('[data-structure-id="'+extra+'"]'),'Delete Layer');
  await page.getByRole('alertdialog').getByRole('button',{name:'Delete',exact:true}).click();await page.waitForTimeout(180);assert.equal((await layerIds()).length,4);
  await contextAction(matrix.locator('.matrix-frame').last(),'Delete Frame');
  assert.equal(await page.getByRole('alertdialog').count(),1);await page.getByRole('alertdialog').getByRole('button',{name:'Delete',exact:true}).click();await page.waitForTimeout(180);assert.equal((await frameIds()).length,11);
  await cell(1,0).click();await cell(2,2).click({modifiers:['Shift']});
  const buttons=await page.locator('.status-bar button').evaluateAll(buttons=>buttons.map(b=>b.getAttribute('aria-label')));
  const side=buttons.indexOf('Toggle primary sidebar');
  if(side>=0)assert.equal(buttons[side+1],'Toggle timeline matrix','layout button follows sidebar toggle');
  await page.screenshot({path:'output/timeline-matrix-sketch.png'});
  await page.setViewportSize({width:900,height:700});
  assert(await cell(0,0).isVisible());
  const tools=matrix.getByRole('toolbar',{name:'Timeline actions'});
  assert((await tools.boundingBox()).height<48,'toolbar remains a single row');
  await tools.getByRole('button',{name:'Create',exact:true}).click();
  await page.getByRole('menuitem',{name:'Empty frame',exact:true}).waitFor();await page.keyboard.press('Escape');
  await page.getByRole('menu',{name:'Create artwork structure',exact:true}).waitFor({state:'hidden'});
  await tools.getByRole('button',{name:'Cels',exact:true}).click();
  await page.getByRole('menuitem',{name:'Clear selected cels',exact:true}).waitFor();await page.keyboard.press('Escape');
  await page.getByRole('menu',{name:'Selected cels',exact:true}).waitFor({state:'hidden'});
  // Edge scrolling extends a drag beyond the frames initially visible in a narrow pane.
  const viewport=matrix.locator('.frame-scroll-area-viewport');
  await viewport.evaluate(el=>{el.scrollLeft=0;});
  const start=await cell(2,0).boundingBox(), edge=await viewport.boundingBox();
  await page.mouse.move(start.x+start.width/2,start.y+start.height/2);await page.mouse.down();
  await page.mouse.move(edge.x+edge.width-4,start.y+start.height/2,{steps:10});await page.waitForTimeout(400);await page.mouse.up();
  assert(await viewport.evaluate(el=>el.scrollLeft>0),'drag auto-scrolls horizontally');
  assert(await matrix.locator('[role=gridcell][aria-selected=true]').count()>4);
  await cell(1,0).scrollIntoViewIfNeeded();await cell(1,0).click();await cell(2,2).click({modifiers:['Shift']});
  await page.screenshot({path:'output/timeline-matrix-compact.png'});
  await page.getByRole('button',{name:'Toggle primary sidebar',exact:true}).click();
  await page.setViewportSize({width:480,height:700});
  assert((await tools.boundingBox()).height<48,'very small toolbar remains one row');
  await tools.getByRole('button',{name:'Playback',exact:true}).click();
  await page.getByRole('menuitem',{name:'Play forward',exact:true}).waitFor();await page.keyboard.press('Escape');
  await page.getByRole('menu',{name:'Timeline playback',exact:true}).waitFor({state:'hidden'});
  await page.screenshot({path:'output/timeline-matrix-small.png'});
  await page.setViewportSize({width:310,height:700});
  await tools.getByRole('button',{name:'Actions',exact:true}).click();
  await page.getByRole('menuitem',{name:'Playback',exact:true}).waitFor();await page.keyboard.press('Escape');
  await page.getByRole('menu',{name:'Timeline actions menu',exact:true}).waitFor({state:'hidden'});
  await page.getByRole('button',{name:'Hide panel',exact:true}).click();
  await page.getByRole('button',{name:'Timeline',exact:true}).waitFor();
  const headerContract=await page.evaluate(async()=>{
    const {verifyPaneHeader}=await import('/scripts/fixtures/pane-header.ts');return verifyPaneHeader();
  });
  assert.deepEqual(headerContract,{hidden:0,standard:1,maximized:true,reopened:0,moved:0,isolated:true});
  assert.deepEqual(errors,[]);
  console.log('PASS: cel clipboard snapshots, cross-layer/frame copy/paste, group paste, single toolbar, responsive menus, selection, clearing, links, structure operations, and layout switching.');
 } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
