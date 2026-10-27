const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const server=require('node:child_process').spawn(process.execPath,['node_modules/vite/bin/vite.js','--host','127.0.0.1','--port','1477','--strictPort'],{stdio:'pipe',windowsHide:true});
 await new Promise((resolve,reject)=>{server.stdout.on('data',d=>{if(String(d).includes('Local:'))resolve()});server.on('error',reject);server.on('exit',code=>reject(new Error('Vite exited '+code)));});
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try {
  const page=await browser.newPage({viewport:{width:1280,height:900}}),errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  page.setDefaultTimeout(12000);
  await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace('let opened=false','let opened=true').replace("frames=[{id:'first',duration_ms:100}]","frames=Array.from({length:30},(_,i)=>({id:'frame-'+i,duration_ms:100}))").replace("layers=[{id:'layer',name:'Layer',visible:true,opacity:1}]","layers=Array.from({length:4},(_,i)=>({id:'layer-'+i,name:'Layer '+i,visible:true,opacity:1}))").replace('revision=0','revision=1')}));
  await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async cb=>{window.testQuit=()=>cb({preventDefault(){}});return ()=>{}},close:async()=>{window.testClosed=true}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto('http://127.0.0.1:1477');
  await page.locator('canvas.artwork').waitFor();
  const list=page.locator('#side-panel .structure-list:visible');
  const selected=()=>list.locator('.preview-row.selected').locator('..').getAttribute('data-reorder-id');
  await page.keyboard.press('Control+2');await list.waitFor();
  const small=await list.evaluate(el=>({client:el.clientHeight,scroll:el.scrollHeight}));console.log('SMALL',small);
  assert(small.scroll<=small.client+1,'four layers should fit without scrollbar');
  await list.locator('.preview-row-select').first().click();
  assert.equal(await selected(),'layer-3');
  await page.keyboard.press('ArrowDown');assert.equal(await selected(),'layer-2');
  await page.keyboard.press('ArrowUp');assert.equal(await selected(),'layer-3');
  await page.keyboard.press('ArrowUp');assert.equal(await selected(),'layer-3');
  await page.keyboard.press('Control+3');await list.waitFor();
  await list.locator('.preview-row-select').first().click();
  await page.keyboard.press('ArrowDown');assert.equal(await selected(),'frame-1');
  await page.keyboard.press('ArrowUp');assert.equal(await selected(),'frame-0');
  async function scrubToBottom(expected) {
    await list.hover();
    const box=await list.boundingBox();
    const bar=page.locator('.frame-overlay-scrollbar.y').filter({visible:true});
    await bar.first().waitFor();
    const tracks=await bar.all();let target;
    for(const track of tracks){const b=await track.boundingBox();if(Math.abs(b.x+b.width-box.x-box.width)<16){target=track;break;}}
    assert(target,'list scrollbar exists');const b=await target.boundingBox();
    await page.mouse.move(b.x+b.width/2,b.y+8);await page.mouse.down();
    await page.mouse.move(b.x+b.width/2,b.y+b.height-2,{steps:10});
    await page.waitForFunction(expected=>[...document.querySelectorAll('#side-panel .structure-list')].find(el=>el.getBoundingClientRect().height>0)?.querySelector('.preview-row.selected')?.parentElement?.dataset.reorderId===expected,expected);
    await page.mouse.up();assert.equal(await selected(),expected);
  }
  await scrubToBottom('frame-29');
  await list.locator('.preview-row-select').last().focus();await page.keyboard.press('ArrowDown');assert.equal(await selected(),'frame-29');
  await page.keyboard.press('ArrowUp');assert.equal(await selected(),'frame-28');
  await page.setViewportSize({width:1100,height:260});await page.keyboard.press('Control+2');
  await scrubToBottom('layer-0');
  await page.setViewportSize({width:1100,height:900});
  assert(await list.evaluate(el=>el.scrollHeight<=el.clientHeight+1));
  assert.deepEqual(errors,[]);
  console.log('PASS: arrow selection and boundaries in Layers/Timeline; scrollbar scrubbing selects first-to-last; four fitting layers have no scrollbar.');
 } finally {await browser.close();server.kill();}
})().catch(e=>{console.error(e);process.exit(1)});
