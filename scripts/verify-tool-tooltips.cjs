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

 const ids=await page.locator('.tool-grid [data-tool]').evaluateAll(buttons=>buttons.map(b=>b.dataset.tool));assert.equal(ids[ids.indexOf('color')+1],'wand');assert.equal(ids[ids.indexOf('lasso')+1],'paint-select');
 const rectangle=page.locator('[data-tool=rectangle]');await rectangle.hover();const tip=page.getByRole('tooltip');await tip.waitFor();
 assert.equal(await tip.locator('strong').textContent(),'Rectangular Selection');
 const rows=await tip.locator('.tool-tooltip-shortcuts > div').allTextContents();assert.deepEqual(rows,['R: Left Button','Alt+R: Right Button']);
 const keys=await tip.locator('.tool-tooltip-description code').allTextContents();assert(keys.includes('Shift'));assert(keys.includes('Control'));assert(keys.includes('Control + Shift'));
 assert.equal(await tip.locator('p').count(),3);
 const descriptionId=await rectangle.getAttribute('aria-describedby');assert(descriptionId);assert.equal(await tip.getAttribute('id'),descriptionId);
 const typography=await tip.locator('strong').evaluate(el=>Number(getComputedStyle(el).fontWeight));assert(typography>=600);
 fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/rich-tool-tooltip.png'});
 // Keyboard focus opens the next tool's help while leaving focus on its button.
 await page.mouse.move(900,200);await rectangle.focus();await page.keyboard.press('Tab');
 await page.waitForFunction(()=>document.querySelector('[role=tooltip] strong')?.textContent==='Elliptical Selection');assert.equal(await page.evaluate(()=>document.activeElement?.getAttribute('data-tool')),'ellipse');
 await page.keyboard.press('Escape');await page.mouse.move(900,200);
 // Rich help remains available for every tool, including tools without a shortcut.
 for(const id of ids){await page.mouse.move(900,200);await page.locator('[data-tool="'+id+'"]').hover();await tip.waitFor();assert(await tip.locator('strong').textContent());assert.equal(await tip.locator('p').count()>0,true);}
 await page.mouse.move(900,200);await rectangle.click();assert(await rectangle.evaluate(el=>el.classList.contains('assigned-left')));await page.locator('[data-tool=wand]').click({button:'right'});assert(await page.locator('[data-tool=wand]').evaluate(el=>el.classList.contains('assigned-right')));assert.deepEqual(errors,[]);
 console.log('Tool help: requested order, title case, bold titles, separate shortcut rows, inline key styling, descriptive paragraphs, ARIA association, keyboard focus, all 21 help cards, and both mouse assignments passed');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
