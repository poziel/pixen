// Run against a Vite server. Storage is mocked; gestures and pixel operations are real.
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try{
  const page=await browser.newPage({viewport:{width:1280,height:900}}),errors=[];
  page.setDefaultTimeout(10000);page.on('pageerror',e=>{errors.push(e.message);console.error(e.message)});page.on('console',m=>{if(m.type()==='error')console.error(m.text())});
  await page.route('**/src/services/backend.ts',route=>route.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/manual-tools.js','utf8')}));
  await page.route('**/@tauri-apps_api_window.js*',route=>route.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',route=>route.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1459');
  await page.locator('.pane-welcome-actions').getByRole('button',{name:/Open artwork/}).click();
  if (!await page.locator('.color-picker').isVisible()) await page.getByRole('button',{name:'Toggle primary sidebar',exact:true}).click();await page.getByRole('button',{name:'Tools',exact:true}).click();
  await page.locator('.color-picker').waitFor();
  const toolbar=page.getByRole('toolbar',{name:'Drawing tools'});
  await toolbar.waitFor();
  assert.equal(await page.locator('.pixel-tools .tool-grid').count(),0);
  const buttons=await toolbar.locator('button').all();
  const boxes=await Promise.all(buttons.map(button=>button.boundingBox()));
  assert(boxes.length>=13);
  assert(boxes.every((box,i)=>box && Math.abs(box.y-boxes[0].y)<1 && (!i||box.x>boxes[i-1].x)));
  const dock=await page.locator('[data-dock-id="pixen.drawing-tools"]').boundingBox();
  const canvas=await page.locator('.canvas-stage').boundingBox();
  const workspace=await page.locator('.workspace-region').boundingBox();
  assert(dock.y>=workspace.y&&dock.y+dock.height<=workspace.y+workspace.height+1);
  assert.equal(await page.locator('[data-dock-host="pixen.drawing-tools"]').evaluate(el=>el.closest('[data-dock-placement]').dataset.dockPlacement),'overlay');



  for(const width of [1280,700,480]) {
    await page.setViewportSize({width,height:900});await page.waitForTimeout(100);
    const surface=page.locator('[data-dock-id="pixen.drawing-tools"]');
    const b=await surface.boundingBox(),area=await page.locator('.canvas-stage').boundingBox();
    assert(b.y>=area.y && b.y+b.height<=area.y+area.height+1);
    assert(Math.abs(b.x+b.width/2-area.x-area.width/2)<2,'toolbar should be horizontally centered');
    const styles=await surface.evaluate(el=>{const s=getComputedStyle(el);return [s.backgroundColor,s.borderTopWidth,s.padding,s.boxShadow]});
    assert.deepEqual(styles,['rgba(0, 0, 0, 0)','0px','0px','none']);
    await page.getByRole('button',{name:'Toggle drawing toolbar',exact:true}).click();await page.waitForTimeout(50);
    assert.deepEqual(await page.locator('.canvas-stage').boundingBox(),area,'hiding overlay must not resize canvas');
    await page.getByRole('button',{name:'Toggle drawing toolbar',exact:true}).click();await surface.waitFor();
  }
  await page.setViewportSize({width:1280,height:900});await page.waitForTimeout(100);
  const surface=page.locator('[data-dock-id="pixen.drawing-tools"]'),b=await surface.boundingBox(),area=await page.locator('.canvas-stage').boundingBox();
  assert.equal(await page.evaluate(({x,y})=>!!document.elementFromPoint(x,y)?.closest('[data-dock-id],.frame-dock-slot'),{x:area.x+4,y:b.y+b.height/2}),false);
  assert.deepEqual(errors,[]);
  require('node:fs').mkdirSync('target/ui-verification',{recursive:true});
  await page.screenshot({path:'target/ui-verification/floating-toolbar.png'});
  console.log('PASS: unstyled overlay toolbar centered at all tested widths, stable canvas geometry, transparent lanes pass clicks through.');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exit(1)});
