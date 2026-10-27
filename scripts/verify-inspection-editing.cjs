const {chromium}=require('playwright');const fs=require('node:fs');const assert=require('node:assert/strict');
(async()=>{const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});try{
 const page=await browser.newPage({viewport:{width:1440,height:960}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
 let fixture=fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace("frames=[{id:'first',duration_ms:100}]","frames=[{id:'first',duration_ms:100},{id:'middle',duration_ms:50},{id:'last',duration_ms:55}]");
 fixture="import {inspectionPreview} from '/src/editor/tools/inspectionPreview.ts';\n"+fixture;
 fixture=fixture.replace("if(command==='render'){",`if(command==='render'){
   const request=args.request,transparent=request.hidden_layers?.includes('layer');
   function source(id){const data=id==='middle'?pixels.slice():new Uint8ClampedArray(1024);if(id==='first')data.set([255,255,255,255],0);if(id==='last')data.set([255,255,255,255],1020);return transparent?new Uint8ClampedArray(1024):data;}
   const current=source(request.frame_id),index=frames.findIndex(f=>f.id===request.frame_id);
   const prior=request.reference_frame_id??frames[index-1]?.id??(request.mode==='frame_comparison'?request.frame_id:undefined);
   const next=!request.reference_frame_id?frames[index+1]?.id:undefined;
   const rgba=inspectionPreview(current,request.mode,prior?source(prior):undefined,next?source(next):undefined);
   const canvas=document.createElement('canvas');canvas.width=16;canvas.height=16;canvas.getContext('2d').putImageData(new ImageData(rgba,16,16),0,0);
   return {width:16,height:16,png:canvas.toDataURL().split(',')[1]};
  }
  if(command==='unused_render'){
 `);
 await page.route('**/src/services/backend.ts*',r=>r.fulfill({contentType:'text/javascript',body:fixture}));
 await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
 await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
 await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1487',{waitUntil:'commit'});
 await page.locator('.pane-welcome-actions').getByRole('button',{name:/Open artwork/}).click();
 await page.getByRole('button',{name:'Toggle timeline matrix',exact:true}).click();
 await page.locator('[data-cell="0-1"] button').click();
 await page.getByRole('toolbar',{name:'Drawing tools'}).locator('[data-tool=pencil]').click();
 await page.getByLabel('Hex color, optional alpha').fill('#12ab34ff');await page.getByLabel('Hex color, optional alpha').press('Tab');
 const canvas=page.locator('canvas.artwork:not(.selection-overlay)');await page.waitForFunction(()=>document.querySelector('canvas.artwork')?.width===16);
 for(const [mode,x] of [['Onion skin',4],['Frame comparison',6]]){
  await page.getByRole('menuitem',{name:'Inspect',exact:true}).click();await page.getByRole('menuitemcheckbox',{name:mode,exact:true}).click();await page.waitForTimeout(150);
  const box=await canvas.boundingBox();const p={x:box.x+(x+.5)*box.width/16,y:box.y+4.5*box.height/16};
  const before=await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length);
  await page.mouse.move(p.x,p.y);await page.mouse.down();await page.waitForTimeout(150);await page.mouse.move(p.x+box.width/16,p.y,{steps:3});await page.mouse.up();await page.waitForTimeout(250);
  assert.equal(await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length),before+1,mode+' permits drawing');
  assert.deepEqual(await page.evaluate(x=>window.toolPixels().slice((4*16+x)*4,(4*16+x)*4+4),x),[18,171,52,255]);
  assert.deepEqual(await page.evaluate(()=>window.toolPixels().slice(0,4)),[0,0,0,0],'ghost/reference pixels are never painted into the cel');
  const view=await canvas.evaluate(c=>Array.from(c.getContext('2d').getImageData(0,0,1,1).data));
  assert.equal(view[0],255,'inspection view stays active after the stroke');
 }
 assert.deepEqual(errors,[]);console.log('PASS: drawing in onion-skin and comparison views edits original cel pixels and keeps overlays display-only.');
}finally{await browser.close();}})().catch(e=>{console.error(e);process.exitCode=1;});
