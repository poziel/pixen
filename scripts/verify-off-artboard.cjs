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

 // Zoom out to leave room for off-artboard interaction within the workspace.
 let box=await art.boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);for(let i=0;i<6;i++){await page.mouse.wheel(0,100);await page.waitForTimeout(40);}
 await tool('pencil');let outside=await point(-2.5,-1.5);await page.mouse.move(outside.x,outside.y);
 assert.equal(await page.locator('.tool-cursor [data-slot=left]').getAttribute('data-pixel-x'),'-3');
 assert.equal(await page.locator('.tool-cursor').evaluate(el=>getComputedStyle(el).overflow),'visible');
 await page.locator('[data-tool=eraser]').click();outside=await point(17.5,17.5);await page.mouse.move(outside.x,outside.y);assert.equal(await page.locator('.tool-cursor [data-slot=left]').count(),1);
 // A brush stroke entirely outside must never edit artwork.
 await page.mouse.down();await page.mouse.move(outside.x+5,outside.y+5);await page.mouse.up();await page.waitForTimeout(100);assert.equal(await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length),0);
 await tool('rectangle');await draw(2,2,5,5);
 let a=await point(3.5,3.5),b=await point(-4.5,3.5);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:6});assert.deepEqual(await bounds(),{x:-6,y:2,width:4,height:4});assert((await page.locator('.selection-ants').getAttribute('d')).includes('M-6'));
 // Return during the same drag, before clipping is committed.
 await page.mouse.move(a.x,a.y,{steps:6});await page.mouse.up();assert.deepEqual(await bounds(),{x:2,y:2,width:4,height:4});
 await page.mouse.move(a.x,a.y);await page.mouse.down();b=await point(.5,3.5);await page.mouse.move(b.x,b.y,{steps:6});assert.equal((await bounds()).x,-1);await page.mouse.up();assert.deepEqual(await bounds(),{x:0,y:2,width:3,height:4});
 // A fully external drop removes the mask.
 a=await point(1.5,3.5);b=await point(-5.5,3.5);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:6});assert((await bounds()).x<0);await page.mouse.up();assert.equal((await bounds()).width,null);
 // Drawing and resizing selections preview beyond the artboard, then clip on release.
 a=await point(12.5,4.5);b=await point(19.5,8.5);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:6});assert.deepEqual(await bounds(),{x:12,y:4,width:8,height:5});await page.mouse.up();assert.deepEqual(await bounds(),{x:12,y:4,width:4,height:5});
 await reset();await draw(-2,2,3,5);assert.deepEqual(await bounds(),{x:0,y:2,width:4,height:4});
 const h=await page.locator('[data-selection-handle=w]').boundingBox();a={x:h.x+h.width/2,y:h.y+h.height/2};b=await point(-3,4);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:5});assert((await bounds()).x<0);await page.mouse.up();assert.equal((await bounds()).x,0);
 fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/off-artboard-indicators.png'});
 assert.equal(await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length),0);assert.deepEqual(errors,[]);
 console.log('Off-artboard: pencil/eraser outlines, safe external strokes, unclipped selection drag/resize/drawing, return before drop, clipping on release, and external selection starts passed');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
