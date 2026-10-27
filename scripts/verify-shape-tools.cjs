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

 // Nineteen real Frame buttons render locally bundled SVG icons.
 assert.equal(await page.locator('.tool-grid button.frame-icon-button svg').count(),21);
 const calls=()=>page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length);
 const pixel=(x,y)=>page.evaluate(([x,y])=>window.toolPixels().slice((y*16+x)*4,(y*16+x)*4+4),[x,y]);
 async function undo(){await page.locator('.canvas-stage').focus();await page.keyboard.press('Control+z');await page.waitForTimeout(120);}
 async function shape(id,a,b,mods=[]){await tool(id);const before=await calls();const p=await point(a[0]+.5,a[1]+.5),q=await point(b[0]+.5,b[1]+.5);for(const m of mods)await page.keyboard.down(m);await page.mouse.move(p.x,p.y);await page.mouse.down();await page.mouse.move(q.x,q.y,{steps:6});await page.waitForTimeout(100);assert.equal(await calls(),before,'drag is preview only');await page.mouse.up();for(const m of [...mods].reverse())await page.keyboard.up(m);await page.waitForFunction(count=>window.testCalls.filter(c=>c.command==='paint_pixels').length===count+1,before);await page.waitForTimeout(100);}
 await shape('line',[5,5],[9,6],['Control']);assert.equal((await pixel(1,4))[3],255);assert.equal((await pixel(9,6))[3],255);await undo();assert(await page.evaluate(()=>window.toolPixels().every(n=>n===0)));
 await shape('line',[3,3],[10,4],['Shift']);assert.equal((await pixel(10,3))[3],255);assert.equal((await pixel(10,4))[3],0);await undo();
 await shape('draw-rectangle',[3,3],[6,5],['Control','Shift']);assert.equal((await pixel(0,0))[3],255);assert.equal((await pixel(3,3))[3],0);await undo();
 await shape('draw-ellipse',[2,2],[10,10]);assert.equal((await pixel(6,2))[3],255);assert.equal((await pixel(6,6))[3],0);await undo();
 // Right-button color and selection clipping reuse the painting contract.
 await page.locator('[data-tool=line]').click({button:'right'});let p=await point(2.5,12.5),q=await point(8.5,12.5);let n=await calls();await page.mouse.move(p.x,p.y);await page.mouse.down({button:'right'});await page.mouse.move(q.x,q.y);await page.mouse.up({button:'right'});await page.waitForFunction(count=>window.testCalls.filter(c=>c.command==='paint_pixels').length>count,n);await page.waitForTimeout(100);assert.deepEqual(await pixel(5,12),[255,255,255,255]);await undo();
 await tool('rectangle');await draw(4,4,6,6);await shape('line',[1,5],[10,5]);assert.equal((await pixel(3,5))[3],0);assert.equal((await pixel(5,5))[3],255);assert.equal((await pixel(7,5))[3],0);await undo();await reset();
 // Alt moves the origin during a shape instead of starting color sampling.
 await tool('line');p=await point(2.5,2.5);q=await point(6.5,4.5);n=await calls();await page.mouse.move(p.x,p.y);await page.mouse.down();await page.mouse.move(q.x,q.y);await page.keyboard.down('Alt');await page.mouse.move(q.x+45,q.y+45);assert.equal(await page.locator('.color-sampler').count(),0);await page.keyboard.up('Alt');await page.keyboard.press('Escape');await page.mouse.up();assert.equal(await calls(),n);
 // Curve points do not commit until double-click; dragging adds tangents.
 await tool('curve');p=await point(2.5,8.5);q=await point(4.5,3.5);await page.mouse.move(p.x,p.y);await page.mouse.down();await page.mouse.move(q.x,q.y);await page.mouse.up();await page.waitForTimeout(100);assert.equal(await calls(),n);assert.equal(await page.locator('.shape-tangent-guide').count(),1);
 p=await point(8.5,8.5);await page.mouse.click(p.x,p.y);await page.keyboard.press('Control');assert.equal(await calls(),n);p=await point(12.5,8.5);await page.mouse.dblclick(p.x,p.y);await page.waitForFunction(count=>window.testCalls.filter(c=>c.command==='paint_pixels').length===count+1,n);await page.waitForTimeout(100);assert(await page.evaluate(()=>window.toolPixels().some(v=>v!==0)));await undo();assert(await page.evaluate(()=>window.toolPixels().every(v=>v===0)));
 // Four basis points finish an isometric box in one undo step.
 await tool('isometric');n=await calls();for(const [x,y]of [[6.5,7.5],[10.5,9.5],[2.5,9.5]]){p=await point(x,y);await page.mouse.click(p.x,p.y);}await page.waitForTimeout(100);assert.equal(await calls(),n);p=await point(6.5,3.5);await page.mouse.click(p.x,p.y);await page.waitForFunction(count=>window.testCalls.filter(c=>c.command==='paint_pixels').length===count+1,n);await page.waitForTimeout(100);assert.equal((await pixel(6,3))[3],255);assert.equal((await pixel(10,5))[3],255);await undo();assert(await page.evaluate(()=>window.toolPixels().every(v=>v===0)));
 // Tool assignment shortcuts retain their left/right distinction.
 await page.keyboard.press('l');assert(await page.locator('[data-tool=line]').evaluate(el=>el.classList.contains('assigned-left')));await page.keyboard.press('Alt+c');assert(await page.locator('[data-tool=draw-ellipse]').evaluate(el=>el.classList.contains('assigned-right')));
 fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/shape-tools.png'});assert.deepEqual(errors,[]);
 console.log('Shape tools: line mirroring/snapping, rectangles, ellipses, Alt origins, curve tangents/removal/completion, isometric basis, right colors, clipping, cancellation, shortcuts and single-step undo passed');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
