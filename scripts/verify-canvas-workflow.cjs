const {chromium}=require('playwright');const assert=require('node:assert/strict');const fs=require('node:fs');
(async()=>{const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});try{
 const page=await browser.newPage({viewport:{width:1100,height:900}}),errors=[];page.on('pageerror',e=>{errors.push(e.message);console.error(e.message)});page.setDefaultTimeout(10000);
 await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace('let opened=false','let opened=true')}));
 await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
 await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
 await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1463');
 const art=page.locator('canvas.artwork:not(.selection-overlay)');await art.waitFor();await page.waitForFunction(()=>document.querySelector('canvas.artwork')?.width===16);
 const overlay=page.locator('.selection-boundary');
 const bounds=()=>overlay.evaluate(el=>Object.fromEntries(['x','y','width','height'].map(k=>[k,el.hasAttribute(`data-selection-${k}`)?Number(el.getAttribute(`data-selection-${k}`)):null])));
 async function point(x,y){const b=await art.boundingBox();return {x:b.x+x*b.width/16,y:b.y+y*b.height/16};}
 async function drag(a,b,mods=[]){for(const m of mods)await page.keyboard.down(m);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:6});await page.mouse.up();for(const m of [...mods].reverse())await page.keyboard.up(m);}
 async function draw(x,y,r,b,mods=[]){await drag(await point(x+.5,y+.5),await point(r+.5,b+.5),mods);}
 async function tool(id){await page.locator(`[data-tool="${id}"]`).click();}
 async function reset(){await page.locator('.canvas-stage').focus();await page.keyboard.press('Control+d');}
 async function handle(id,dx,dy,mods=[]){const box=await page.locator(`[data-selection-handle="${id}"]`).boundingBox(),a={x:box.x+box.width/2,y:box.y+box.height/2},b=await art.boundingBox();for(const m of mods)await page.keyboard.down(m);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(a.x+dx*b.width/16,a.y+dy*b.height/16,{steps:6});await page.mouse.up();for(const m of [...mods].reverse())await page.keyboard.up(m);}

 for(const [i,label]of ['Tools','Layers','Timeline','AI chat'].entries()){
  await page.keyboard.press('Control+'+(i+1));
  await page.waitForFunction(label=>document.querySelector('#side-panel')?.getAttribute('aria-label')===label,label);
 }
 await page.keyboard.press('Control+1');await page.waitForFunction(()=>document.activeElement?.classList.contains('canvas-stage'));
 const dock=page.locator('[data-dock-host="pixen.drawing-tools"]');assert.equal(await dock.evaluate(el=>el.closest('[data-dock-placement]').dataset.dockPlacement),'inside');
 await tool('pencil');await page.waitForFunction(()=>document.activeElement?.classList.contains('canvas-stage'));
 for(const key of ['Enter','Space']){
  await tool('pencil');await page.waitForFunction(()=>document.activeElement?.classList.contains('canvas-stage'));
  const before=await art.evaluate(el=>el.style.transform);await page.keyboard.down(key);
  await page.waitForFunction(()=>document.querySelector('[data-tool=pan]')?.classList.contains('assigned-left'));
  const a=await point(4.5,4.5);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(a.x+25,a.y+10,{steps:5});await page.mouse.up();await page.keyboard.up(key);
  assert.notEqual(await art.evaluate(el=>el.style.transform),before);
 }
 // Menu commands return focus after menu dismissal, so the next hold can pan.
 await page.getByRole('menuitem',{name:'View',exact:true}).click();await page.getByRole('menuitem',{name:/Fit canvas/}).click();
 await page.waitForFunction(()=>document.activeElement?.classList.contains('canvas-stage'));
 const hex=page.getByLabel('Hex color, optional alpha');await hex.focus();await page.keyboard.down('Space');assert.equal(await page.locator('[data-tool=pan].assigned-left').count(),0);await page.keyboard.up('Space');await hex.press('Escape');
 await tool('rectangle');await draw(3,3,6,6);const a=await point(4.5,4.5),b=await point(5.5,5.5);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y);
 assert(await page.locator('.selection-guides line').count());
 assert(await page.locator('.selection-guides line').evaluateAll(lines=>lines.every(line=>{const expected=document.createElement('span');expected.style.color=getComputedStyle(document.querySelector('.workbench-layout')).getPropertyValue('--frame-accent');document.body.append(expected);const color=getComputedStyle(expected).color;expected.remove();return getComputedStyle(line).stroke===color;})));
 fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/canvas-workflow.png'});await page.mouse.up();assert.deepEqual(errors,[]);
 console.log('Canvas workflow: Ctrl+1–4 views, inside dock, focus after tools/menu commands, Enter/Space panning, text entry, and accent guides passed');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
