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
  await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace('let opened=false','let opened=true').replace('revision=0','revision=1')}));
  await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async cb=>{window.testQuit=()=>cb({preventDefault(){}});return ()=>{}},close:async()=>{window.testClosed=true}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.addInitScript(() => {
    let fullscreen = null;
    window.fullscreenCalls = 0;
    Object.defineProperty(document, 'fullscreenEnabled', {get:()=>true});
    Object.defineProperty(document, 'fullscreenElement', {get:()=>fullscreen});
    Element.prototype.requestFullscreen = async function() { fullscreen=this;window.fullscreenCalls++;document.dispatchEvent(new Event('fullscreenchange')); };
    document.exitFullscreen = async function() { fullscreen=null;window.fullscreenCalls++;document.dispatchEvent(new Event('fullscreenchange')); };
  });
  await page.goto('http://127.0.0.1:1477');
  await page.locator('canvas.artwork').waitFor();
  const spaces=await page.locator('.titlebar-drag').all();
  let space;
  for(const item of spaces) { const b=await item.boundingBox();if(b?.width>10){space=item;break;} }
  assert(space,'empty titlebar area exists');
  await space.dblclick({position:{x:5,y:10}});
  await page.waitForFunction(()=>window.fullscreenCalls===1);
  assert(await page.evaluate(()=>!!document.fullscreenElement));
  await space.dblclick({position:{x:5,y:10}});
  await page.waitForFunction(()=>window.fullscreenCalls===2);
  assert.equal(await page.evaluate(()=>!!document.fullscreenElement),false);
  await page.getByRole('menuitem',{name:'View',exact:true}).dblclick();
  assert.equal(await page.evaluate(()=>window.fullscreenCalls),2);
  assert.deepEqual(errors,[]);
  console.log('PASS: empty titlebar double-click toggles fullscreen on/off; menu-item double-click does not toggle fullscreen.');
 } finally {await browser.close();server.kill();}
})().catch(e=>{console.error(e);process.exit(1)});
