// Browser interaction coverage with mocked native storage; native edits have Rust tests.
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try {
  const page=await browser.newPage({viewport:{width:1200,height:850}}), errors=[];
  page.on('pageerror', e=>errors.push(e.message));
  let fixture=fs.readFileSync('scripts/fixtures/manual-tools.js','utf8');
  fixture=fixture.replace('frames,layers,path:', 'frames:structuredClone(frames),layers:structuredClone(layers),path:');
  fixture=fixture.replace("if(command==='snapshot')", `if(command==='edit_structure') {
    if(args.generation!==generation)throw new Error('stale edit');
    for(const op of (Array.isArray(args.operation)?args.operation:[args.operation])) {
      const isLayer=op.type.endsWith('_layer'),list=isLayer?layers:frames;
      const id=isLayer?op.layer_id:op.frame_id,index=list.findIndex(item=>item.id===id);
      if(op.type.startsWith('add_'))list.splice(op.index,0,isLayer?{id,name:op.name,visible:true,opacity:1}:{id,duration_ms:op.duration_ms});
      if(op.type.startsWith('duplicate_'))list.splice(op.index,0,{...list[index],id:isLayer?op.new_layer_id:op.new_frame_id,...(isLayer?{name:op.name}:{})});
      if(op.type.startsWith('rename_'))list[index].name=op.name;
      if(op.type.startsWith('reorder_'))list.splice(op.index,0,list.splice(index,1)[0]);
      if(op.type.startsWith('remove_'))list.splice(index,1);
    }
    generation++;revision++;return;
  }
  if(command==='snapshot')`);
  await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fixture}));
  await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1459');
  await page.locator('.pane-welcome-actions').getByRole('button',{name:/Open artwork/}).click();
  await page.evaluate(()=>document.addEventListener('dragstart',event=>event.preventDefault(),true));
  for (const [kind,view] of [['layer','Layers'],['frame','Timeline']]) {
   await page.getByRole('button',{name:view,exact:true}).click();
   const list=page.locator('.structure-list:visible'), rows=list.locator('.preview-row');
   await list.waitFor();
   const add=page.getByRole('button',{name:`Add ${kind}`,exact:true}), del=page.getByRole('button',{name:`Delete ${kind}`,exact:true});
   assert.equal(await list.getByRole('textbox').count(),0);
   assert(await del.isDisabled());
   await add.click();await page.waitForTimeout(100);assert.equal(await rows.count(),2);
   await rows.filter({hasText:kind==='layer'?'Layer 2':'Frame 2'}).locator('strong').dblclick();
   const input=list.getByRole('textbox');await input.fill('Named '+kind);await input.press('Enter');
   await rows.locator('strong').filter({hasText:'Named '+kind}).waitFor();
   assert.equal(await list.getByRole('textbox').count(),0);
   let target=rows.filter({hasText:'Named '+kind});
   await target.click({button:'right'});await page.getByRole('menuitem',{name:/^Rename/}).click();
   await input.fill('Canceled');await input.press('Escape');
   await target.locator('strong').waitFor();
   await target.click({button:'right'});await page.getByRole('menuitem',{name:'Duplicate',exact:true}).click();
   await rows.filter({hasText:'Named '+kind+' copy'}).waitFor();assert.equal(await rows.count(),3);
   const names=await rows.locator('strong').allTextContents(), at=names.indexOf('Named '+kind);
   assert.equal(names[at+1],'Named '+kind+' copy','duplicate immediately below source');
   await del.click();await page.getByRole('alertdialog').waitFor();assert.equal(await rows.count(),3);
   await page.getByRole('button',{name:'Cancel',exact:true}).click();assert.equal(await rows.count(),3);
   await del.click();await page.getByRole('alertdialog').getByRole('button',{name:'Delete',exact:true}).click();
   await page.waitForTimeout(100);assert.equal(await rows.count(),2);
   // Empty-space menu offers only creation.
   const bounds=await list.boundingBox();await list.click({button:'right',position:{x:bounds.width-8,y:bounds.height-15}});
   assert.equal(await page.getByRole('menu',{name:kind+' actions',exact:true}).getByRole('menuitem').count(),kind==='frame'?2:1);
   await page.getByRole('menuitem',{name:`New ${kind}`,exact:true}).click();await page.waitForTimeout(100);assert.equal(await rows.count(),3);
   // Group selection and real native drag to the end.
   const before=await list.locator('[data-reorder-id]').evaluateAll(items=>items.map(item=>item.dataset.reorderId));
   await rows.nth(0).getByRole('button').last().click();
   await rows.nth(1).getByRole('button').last().click({modifiers:['Control']});
   assert.equal(await list.locator('[data-selected]').count(),2);
   const items=list.locator('[data-reorder-id]');
   const box=await items.nth(2).boundingBox();
   const source=await items.nth(0).boundingBox();
   await page.mouse.move(source.x+source.width/2,source.y+source.height/2);await page.mouse.down();
   await page.mouse.move(box.x+box.width/2,box.y+box.height-2,{steps:8});
   const indicator=list.locator('.frame-reorder-indicator');
   assert(await indicator.isVisible(),'show the insertion bar before releasing');
   const line=await indicator.boundingBox();assert(Math.abs(line.y-(box.y+box.height))<3);assert(line.width>100);
   const preview=page.locator('.frame-reorder-preview');
   assert(await preview.isVisible());assert((await preview.innerText()).includes(kind==='layer'?'2 layers':'2 frames'));
   assert.equal(await preview.locator('.structure-drag-item').count(),2);
   const ghost=await preview.boundingBox();assert(ghost.x>=0 && ghost.y>=0 && ghost.x+ghost.width<=1201 && ghost.y+ghost.height<=851);
   await page.mouse.up();
   assert.equal(await preview.count(),0);
   await page.waitForTimeout(100);
   assert.deepEqual(await list.locator('[data-reorder-id]').evaluateAll(items=>items.map(item=>item.dataset.reorderId)),[before[2],before[0],before[1]]);
   // Context creation is anchored to the clicked row, rather than the active row.
   await rows.nth(0).click({button:'right'});await page.getByRole('menuitem',{name:`New ${kind}`,exact:true}).click();
   await page.waitForTimeout(100);assert.equal(await rows.count(),4);
  }
  assert.deepEqual(errors,[]);
  console.log('PASS: header actions, default names, double-click/context rename, duplicate placement, delete cancel/confirm, blank menu, and multi-row drag for layers and frames.');
 } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exit(1)});
