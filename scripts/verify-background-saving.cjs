// Run against a Vite server. Storage is mocked; gestures and pixel operations are real.
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try{
  const page=await browser.newPage({viewport:{width:1280,height:900}}),errors=[];
  page.setDefaultTimeout(10000);page.on('pageerror',e=>{errors.push(e.message);console.error(e.message)});page.on('console',m=>{if(m.type()==='error')console.error(m.text())});
  let fixture=fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace('return {managed:false,','return {recovery:window.testRecovery??null,managed:false,');
  fixture=fixture.replace("if(command==='snapshot')",`if(command==='save_document'){window.saveStarted=true;await new Promise(resolve=>window.finishSave=resolve);return true;}
  if(command==='read_settings_file')return window.testSettingsFile??null;
  if(command==='write_settings_file'){window.testSettingsFile=args.json;return;}
  if(command==='configure_recovery'){window.recoveryInterval=args.seconds;return;}
  if(command==='snapshot')`);
  await page.route('**/src/services/backend.ts',route=>route.fulfill({contentType:'text/javascript',body:fixture}));
  await page.route('**/@tauri-apps_api_window.js*',route=>route.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',route=>route.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1459',{timeout:60000,waitUntil:'domcontentloaded'});
  await page.locator('.pane-welcome-actions').getByRole('button',{name:/Open artwork/}).click();
  if (!await page.locator('.color-picker').isVisible()) await page.getByRole('button',{name:'Toggle primary sidebar',exact:true}).click();await page.getByRole('button',{name:'Tools',exact:true}).click();
  await page.locator('.color-picker').waitFor();
  await page.waitForFunction(()=>window.recoveryInterval===30);
  await page.locator('.canvas-stage').focus();await page.keyboard.press('Control+,');
  await page.getByRole('dialog', {name:'Settings',exact:true}).getByRole('button', {name:'Recovery',exact:true}).click();
  const interval=page.getByRole('spinbutton',{name:'Save recovery file every (seconds)',exact:true});
  await interval.fill('20');await interval.press('Tab');
  await page.waitForFunction(()=>window.recoveryInterval===20);
  await page.waitForFunction(()=>JSON.parse(window.testSettingsFile??'{}')['pixen.recoveryInterval']===20);
  await page.getByRole('dialog',{name:'Settings',exact:true}).getByRole('button',{name:'Close modal',exact:true}).click();
  await page.getByRole('dialog',{name:'Settings',exact:true}).waitFor({state:'hidden'});
  await page.locator('.canvas-stage').focus();await page.keyboard.press('Control+s');
  await page.waitForFunction(()=>window.saveStarted===true);
  const art=page.locator('canvas.artwork:not(.selection-overlay)');
  const box=await art.boundingBox();
  await page.mouse.move(box.x+box.width*.25,box.y+box.height*.25);await page.mouse.down();await page.mouse.move(box.x+box.width*.35,box.y+box.height*.25,{steps:4});await page.mouse.up();
  await page.waitForFunction(()=>window.testCalls.some(call=>call.command==='paint_pixels'));
  assert.equal(await page.locator('.frame-loading-overlay').count(),0);
  await page.evaluate(()=>window.finishSave());
  await page.getByText('Project saved',{exact:true}).waitFor();
  await page.evaluate(()=>window.testRecovery={writing:false,revision:1,saved_at:1,error:null});
  await page.getByText('Recovery file updated',{exact:true}).waitFor();
  await page.evaluate(()=>window.testRecovery={writing:false,revision:1,saved_at:1,error:'Recovery file could not be updated: disk full'});
  await page.getByText('Recovery file could not be updated: disk full',{exact:true}).waitFor();
  assert.deepEqual(errors,[]);
  require('node:fs').mkdirSync('target/ui-verification',{recursive:true});
  await page.screenshot({path:'target/ui-verification/floating-toolbar.png'});
  console.log('PASS: recovery interval preference, nonblocking Ctrl+S, drawing during file save, and status completion.');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exit(1)});
