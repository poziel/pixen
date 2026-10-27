const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try {
  for(const startupDocument of [false,true]) {
   const page=await browser.newPage({viewport:{width:1100,height:900}}), errors=[];
   page.on('pageerror',e=>errors.push(e.message));
   let fixture=fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace('let opened=false',`let opened=${startupDocument}`);
   fixture += '\nwindow.closeArtwork=()=>{opened=false;};';
   fixture=fixture.replace("if(command==='open_document')", "if(command==='close_document'){opened=false;return true;}\n  if(command==='open_document')");
   await page.route('**/src/services/backend.ts',route=>route.fulfill({contentType:'text/javascript',body:fixture}));
   await page.route('**/@tauri-apps_api_window.js*',route=>route.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
   await page.route('**/@tauri-apps_api_event.js*',route=>route.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
   await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1459');
   const toggle=page.getByRole('button',{name:'Toggle drawing toolbar',exact:true});await toggle.waitFor();
   const sidebar=page.getByRole('button',{name:'Toggle primary sidebar',exact:true});
   const toolbar=page.locator('.drawing-toolbar:visible');
   if(!startupDocument){
    assert.equal(await toolbar.count(),0);
    await sidebar.click();
    await page.locator('.color-picker').waitFor();
    assert(await page.locator('.color-picker').evaluate(el=>el.disabled&&el.inert));
    assert.equal(await page.locator('.tool-parameters:visible').count(),0);
    await toggle.click();assert(await toolbar.evaluate(el=>el.disabled&&el.inert));
    await toggle.click();assert.equal(await toolbar.count(),0);
    await page.getByRole('button',{name:/Open artwork/}).click();
   }
   await toolbar.waitFor();await page.locator('.color-picker').waitFor();
   const toolButtons=toolbar.locator('[data-tool]');
   assert.equal(await toolButtons.count(),21);
   assert.equal(await toolbar.locator('button.frame-icon-button[data-tool] svg').count(),21,'all tools use Frame buttons and SVG icons');
   const geometry=await toolButtons.evaluateAll(buttons=>buttons.map(button=>{const b=button.getBoundingClientRect(),i=button.querySelector('svg').getBoundingClientRect();return {w:b.width,h:b.height,iw:i.width,ih:i.height,dx:Math.abs(b.x+b.width/2-i.x-i.width/2),dy:Math.abs(b.y+b.height/2-i.y-i.height/2)};}));
   assert(geometry.every(b=>b.w===36&&b.h===36&&b.iw===20&&b.ih===20&&b.dx<.1&&b.dy<.1),'icons retain a readable, centered size');
   assert.equal(await toolbar.evaluate(el=>el.disabled),false);
   assert.equal(await page.locator('.color-picker').evaluate(el=>el.disabled||el.inert),false);
   assert.equal(await page.locator('.tool-parameters:visible').count(),0);
   await toggle.click();assert.equal(await toolbar.count(),0);
   await page.waitForTimeout(650);assert.equal(await toolbar.count(),0,'snapshot refresh must preserve manual visibility');
   await toggle.click();await toolbar.waitFor();
   // Show a previously collapsed section and preserve that preference across sidebar toggles.
   const left=page.getByRole('button',{name:'Left tools · Pencil',exact:true});
   const right=page.getByRole('button',{name:'Right tools · Eraser',exact:true});
   assert.equal(await left.evaluate(el=>getComputedStyle(el).color),'rgb(40, 169, 244)');
   assert.equal(await right.evaluate(el=>getComputedStyle(el).color),'rgb(255, 139, 54)');
   await left.click();await page.locator('.tool-parameters:visible').first().waitFor();
   assert.equal(await page.locator('.tool-settings h3').count(),0);
   await page.locator('[data-tool="eraser"]').click();
   await page.getByRole('button',{name:'Left tools · Eraser',exact:true}).waitFor();
   await page.locator('[data-tool="pencil"]').click({button:'right'});
   await page.getByRole('button',{name:'Right tools · Pencil',exact:true}).waitFor();
   assert.equal(await page.locator('.tool-parameters:visible').count(),1,'tool changes preserve collapsed sections');
   await sidebar.click();await sidebar.click();
   assert.equal(await page.locator('.tool-parameters:visible').count(),1);
   const lastRight=page.locator('.status-right [data-status-id]').last();
   assert.equal(await lastRight.getAttribute('data-status-id'),'pixen.toggle-drawing-tools');
   if(startupDocument){fs.mkdirSync('target/ui-verification',{recursive:true});await toolbar.screenshot({path:'target/ui-verification/drawing-toolbar.png'});}
   await page.evaluate(()=>window.closeArtwork());
   await page.waitForTimeout(650);
   assert.equal(await toolbar.count(),0,'closing the last document hides the toolbar');
   await sidebar.click();await page.locator('.color-picker').waitFor();
   assert(await page.locator('.color-picker').evaluate(el=>el.disabled&&el.inert));
   assert.deepEqual(errors,[]);await page.close();
  }
  console.log('Tools layout: empty/document startup, disabled controls, toolbar overrides, and section retention passed');
 } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
