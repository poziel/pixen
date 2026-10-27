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
  await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace('let opened=false','let opened=true').replace("frames=[{id:'first',duration_ms:100}]","frames=Array.from({length:30},(_,i)=>({id:'frame-'+i,duration_ms:100}))").replace('revision=0','revision=1')}));
  await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async cb=>{window.testQuit=()=>cb({preventDefault(){}});return ()=>{}},close:async()=>{window.testClosed=true}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto('http://127.0.0.1:1477');
  await page.locator('canvas.artwork').waitFor();
  await page.keyboard.press('Control+Shift+E');
  const dialog=page.getByRole('dialog');await dialog.waitFor();
  const checkbox=dialog.getByRole('checkbox',{name:'Use background color',exact:true});
  const colors=()=>checkbox.evaluate(el=>({background:getComputedStyle(el).backgroundColor,color:getComputedStyle(el).color,border:getComputedStyle(el).borderColor}));
  await page.mouse.move(10,10);const unchecked=await colors();await checkbox.hover();assert.deepEqual(await colors(),unchecked);
  await checkbox.click();await page.mouse.move(10,10);const checked=await colors();await checkbox.hover();assert.deepEqual(await colors(),checked);
  assert.notEqual(checked.background,unchecked.background);
  const old=await page.addStyleTag({content:'.frame-modal button:hover {background:var(--frame-modal-button-hover-background,#ffffff09)}'});
  assert.notDeepEqual(await colors(),checked,'legacy modal rule must reproduce checkbox hover override');await old.evaluate(el=>el.remove());
  await dialog.getByRole('combobox',{name:'Export',exact:true}).click();await page.getByRole('option',{name:'Animation',exact:true}).click();
  assert.equal(await checkbox.isDisabled(),false);
  await checkbox.click();assert.equal(await checkbox.getAttribute('aria-checked'),'false');
  await dialog.getByRole('spinbutton',{name:'Scale (nearest neighbor)',exact:true}).fill('5');
  await dialog.getByLabel('File name',{exact:true}).fill('slime-5x');
  await dialog.getByLabel('Folder',{exact:true}).fill('D:/Dev/1-codes/pixen/examples/slime');
  assert.equal(await dialog.getByRole('checkbox',{name:'Loop animation',exact:true}).getAttribute('aria-checked'),'true');
  await dialog.getByRole('button',{name:'Export',exact:true}).click();
  const payload=await page.evaluate(()=>window.testCalls.find(call=>call.command==='export_document').args.request.exports[0]);
  assert.equal(payload.scale,5);assert.equal(payload.loopAnimation,true);assert.equal(payload.background,null);assert.equal(payload.mode,'animation');
  assert.deepEqual(errors,[]);
  console.log('PASS: Frame checkbox colors survive hover; legacy rule reproduces issue; GIF request preserves transparent background, 5× scale and looping.');
 } finally {await browser.close();server.kill();}
})().catch(e=>{console.error(e);process.exit(1)});
