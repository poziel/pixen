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
  const dialog=page.getByRole('dialog');
  const colors=button=>button.evaluate(el=>({background:getComputedStyle(el).backgroundColor,color:getComputedStyle(el).color,filter:getComputedStyle(el).filter}));
  async function checkPrimary(name,disabled=false) {
    const button=dialog.getByRole('button',{name,exact:true});await button.waitFor();
    assert.equal(await button.isDisabled(),disabled);
    await page.mouse.move(10,10);const before=await colors(button);
    await button.hover();const after=await colors(button);
    assert.equal(after.background,before.background,name+' background changes incorrectly on hover');
    assert.equal(after.color,before.color,name+' foreground changes incorrectly on hover');
    if(disabled) assert.equal(after.filter,before.filter);
  }
  await page.keyboard.press('Control+n');await checkPrimary('Create');
  const secondary=dialog.getByRole('button',{name:'16 × 16',exact:true});await secondary.hover();
  assert.notEqual((await colors(secondary)).background,'rgba(0, 0, 0, 0)');
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await page.getByRole('menuitem',{name:'File',exact:true}).click();await page.getByRole('menuitem',{name:'Canvas size…',exact:true}).click();
  await checkPrimary('Resize canvas');await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await page.keyboard.press('Control+Shift+E');await checkPrimary('Export',true);
  await dialog.getByLabel('Folder',{exact:true}).fill('D:/exports');await checkPrimary('Export');
  const checkbox=dialog.getByRole('checkbox',{name:'Use background color',exact:true});await checkbox.click();
  await page.mouse.move(10,10);const checked=await colors(checkbox);await checkbox.hover();assert.equal((await colors(checkbox)).background,checked.background);
  assert.deepEqual(errors,[]);
  console.log('PASS: New/Create, Resize canvas, enabled/disabled Export, secondary buttons and checkbox hover styling.');
 } finally {await browser.close();server.kill();}
})().catch(e=>{console.error(e);process.exit(1)});
