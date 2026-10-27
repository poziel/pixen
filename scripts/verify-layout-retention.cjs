// Run against Vite with native document IO mocked.
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try {
  const page=await browser.newPage({viewport:{width:1200,height:900}}),errors=[];
  page.on('pageerror',e=>{errors.push(e.message);console.error(e.message);});page.setDefaultTimeout(30000);
  const fixture=fs.readFileSync('scripts/fixtures/document-workspace.js','utf8')+'\nwindow.openTestArtwork=path=>create(path);';
  await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fixture}));
  await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1467',{waitUntil:'commit',timeout:60000});
  const sidebar=page.getByRole('button',{name:'Toggle primary sidebar',exact:true});
  const matrix=page.getByRole('button',{name:'Toggle timeline matrix',exact:true});
  const panel=page.locator('#side-panel');
  const timeline=page.locator('[data-view-id="pixen.timeline"]');
  const matrixPanel=page.locator('.timeline-matrix:visible');
  async function open(path){await page.evaluate(path=>window.openTestArtwork(path),path);await page.getByRole('tab',{name:path,exact:true,includeHidden:true}).waitFor({state:'attached'});}
  // An already open third sidebar tab must survive the first artwork opening.
  await sidebar.click();await timeline.click();
  await open('first.pixloom');
  assert.equal(await timeline.getAttribute('aria-pressed'),'true');assert(await panel.isVisible());
  assert.equal(await matrixPanel.count(),0);
  await page.keyboard.press('Control+1');
  await page.getByRole('button',{name:'primary color #ffffff',exact:true}).waitFor();
  await page.getByRole('button',{name:'secondary color #000000',exact:true}).waitFor();
  assert.equal(await page.locator('[data-tool=pencil] .tool-assignment-left').count(),1);
  assert.equal(await page.locator('[data-tool=eraser] .tool-assignment-right').count(),1);
  await page.locator('[data-tool=line]').click();
  await page.getByRole('button',{name:'Swap colors (X)',exact:true}).click();
  await page.keyboard.press('Control+3');
  await open('second.pixloom');
  assert.equal(await timeline.getAttribute('aria-pressed'),'true');assert(await panel.isVisible());
  assert.equal(await matrixPanel.count(),0);
  // Opening/closing the matrix leaves the selected sidebar tab and visibility alone.
  await matrix.click();await matrixPanel.waitFor();
  assert.equal(await timeline.getAttribute('aria-pressed'),'true');
  await sidebar.click();assert.equal(await panel.isVisible(),false);
  await open('third.pixloom');
  assert.equal(await panel.isVisible(),false);assert.equal(await matrixPanel.count(),1);
  await sidebar.click();assert.equal(await timeline.getAttribute('aria-pressed'),'true');
  assert.equal(await matrixPanel.count(),1);
  await matrix.click();assert.equal(await matrixPanel.count(),0);
  assert.equal(await timeline.getAttribute('aria-pressed'),'true');assert(await panel.isVisible());
  await sidebar.click();await sidebar.click();
  assert.equal(await timeline.getAttribute('aria-pressed'),'true');
  await page.keyboard.press('Control+1');
  await page.getByRole('button',{name:'primary color #ffffff',exact:true}).waitFor();
  await page.getByRole('button',{name:'secondary color #000000',exact:true}).waitFor();
  assert.equal(await page.locator('[data-tool=pencil] .tool-assignment-left').count(),1);
  assert.equal(await page.locator('[data-tool=eraser] .tool-assignment-right').count(),1);
  assert.deepEqual(errors,[]);
  console.log('PASS: artwork opening, third-tab retention, independent sidebar/matrix toggles, and white/black Pencil/Eraser defaults.');
 } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
