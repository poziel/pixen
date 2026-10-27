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

 await tool('polygon');let p=await point(2.5,2.5);await page.mouse.click(p.x,p.y);p=await point(7.5,2.5);await page.mouse.move(p.x,p.y);
 const preview=page.locator('.polygon-trace-preview');assert((await preview.getAttribute('d')).includes('L7.5 2.5'));
 const before=await preview.getAttribute('d');p=await point(8.5,3.5);await page.mouse.move(p.x,p.y);assert.notEqual(await preview.getAttribute('d'),before);assert.equal((await bounds()).width,null,'preview must not commit selection');
 p=await point(7.5,2.5);await page.mouse.click(p.x,p.y);p=await point(7.5,7.5);await page.mouse.click(p.x,p.y);
 p=await point(2.5,2.5);await page.mouse.move(p.x,p.y);assert.equal(await page.locator('.polygon-start-indicator').getAttribute('data-closable'),'true');assert(await page.getByText('Click to close',{exact:true}).isVisible());
 fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/polygon-close-preview.png'});
 await page.mouse.click(p.x,p.y);assert((await bounds()).width>0);assert.equal(await preview.count(),0);
 await reset();await tool('polygon');for(const [x,y]of [[2.5,2.5],[7.5,2.5]]){p=await point(x,y);await page.mouse.click(p.x,p.y);}p=await point(7.5,7.5);await page.mouse.dblclick(p.x,p.y);assert((await bounds()).width>0);assert.equal(await preview.count(),0);
 await reset();p=await point(2.5,2.5);await page.mouse.click(p.x,p.y);p=await point(7.5,7.5);await page.mouse.move(p.x,p.y);await page.keyboard.press('Escape');assert.equal(await preview.count(),0);assert.equal(await page.locator('.polygon-start-indicator').count(),0);
 assert.equal(await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length),0);assert.deepEqual(errors,[]);
 console.log('Polygon tracing: moving endpoint, complete outline preview, start indicator, click-to-close, double-click completion, cancellation, and unchanged artwork passed');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
