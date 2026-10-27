const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try {
  const page=await browser.newPage({viewport:{width:1100,height:900}});
  await page.route('**/src/services/backend.ts',route=>route.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace('let opened=false','let opened=true')}));
  await page.route('**/@tauri-apps_api_window.js*',route=>route.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',route=>route.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1462');
  await page.locator('.color-picker').waitFor();
  const primary=page.getByRole('button',{name:/^primary color/}),secondary=page.getByRole('button',{name:/^secondary color/});
  const picker=page.getByRole('button',{name:'Pick color from artwork',exact:true});
  // Pick a known pixel directly on the artwork without editing its document.
  const canvas=page.locator('canvas.artwork:not(.selection-overlay)');
  await canvas.waitFor();
  for(const mode of ['alt','button'])for(const button of ['right','left']){
   // Deliberately select the opposite swatch: mouse button must choose the destination.
   await (button==='right'?primary:secondary).click();
   const beforePrimary=await primary.getAttribute('aria-label'),beforeSecondary=await secondary.getAttribute('aria-label');
   const rgb=mode==='alt'?(button==='right'?[18,52,86]:[171,205,239]):(button==='right'?[35,69,103]:[152,118,84]);
   const hex=rgb.map(n=>n.toString(16).padStart(2,'0')).join('');
   if(mode==='alt')await page.keyboard.down('Alt');else await picker.click();
   await canvas.evaluate((el,rgb)=>{const ctx=el.getContext('2d');ctx.fillStyle=`rgb(${rgb.join(',')})`;ctx.fillRect(0,0,el.width,el.height);},rgb);
   const box=await canvas.boundingBox();await page.mouse.click(box.x+box.width/2,box.y+box.height/2,{button});
   if(mode==='alt')await page.keyboard.up('Alt');
   const changed=button==='right'?secondary:primary;
   await page.waitForFunction(({side,hex})=>document.querySelector(`button[aria-label^="${side} color"]`)?.getAttribute('aria-label')?.includes('#'+hex),{side:button==='right'?'secondary':'primary',hex});
   assert.equal(await (button==='right'?primary:secondary).getAttribute('aria-label'),button==='right'?beforePrimary:beforeSecondary,'opposite color stays unchanged');
   assert((await changed.getAttribute('aria-label')).includes(hex));
  }
  assert.equal(await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length),0);
  console.log('Color sampling: Alt and picker button route left/right clicks to primary/secondary independently of selected swatch; no painting');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
