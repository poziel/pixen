// Frontend regression check using mocked document storage and Tauri events.
const {chromium}=require('playwright');const fs=require('fs');const assert=require('node:assert/strict');
(async()=>{const browser=await chromium.launch({channel:'msedge',headless:true});try{const page=await browser.newPage({viewport:{width:1200,height:800}});page.setDefaultTimeout(5000);const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/document-workspace.js','utf8')}));
await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async handler=>{window.dropArtwork=handler;return ()=>{}},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
await page.goto(process.env.PIXEN_TEST_URL || 'http://127.0.0.1:1461');await page.waitForLoadState('networkidle');await page.getByRole('button',{name:/Open artwork/}).first().click();await page.locator('canvas.artwork').waitFor();

await page.evaluate(()=>window.dropArtwork({payload:{type:'drop',paths:['animation.pixloom']}}));await page.locator('[data-status-id="pixen.frame"]').waitFor();
let failures=[];
for(const id of ['pixen.document','pixen.frame','pixen.zoom-out','pixen.zoom','pixen.zoom-in']){
 await page.locator(`[data-status-id="${id}"]`).click({button:'right'});
 await page.waitForTimeout(30);
 await page.waitForTimeout(1200);
 if(!await page.getByRole('menu',{name:'Status bar actions',exact:true}).count()) failures.push(id+' context menu vanished during snapshot updates');
 await page.keyboard.press('Escape');await page.waitForTimeout(100);
}
await page.waitForTimeout(200);await page.locator('.status-message').click({button:'right'});await page.getByRole('menu',{name:'Status bar actions',exact:true}).waitFor();
await page.locator('.status-message').click();await page.waitForTimeout(100);
if(await page.getByRole('menu',{name:'Status bar actions',exact:true}).count())failures.push('same status bar click did not dismiss context menu');
await page.keyboard.press('Escape');
await page.getByRole('button',{name:'Timeline',exact:true}).click();
await page.getByRole('button',{name:'Preview speed',exact:true}).click();await page.waitForTimeout(1200);
if(!await page.getByRole('menu',{name:'View actions',exact:true}).count())failures.push('preview speed dropdown vanished');
await page.getByRole('menuitemcheckbox',{name:/^2/}).click();
await page.getByRole('button',{name:'Preview speed',exact:true}).click();
assert.equal(await page.getByRole('menuitemcheckbox',{name:/^2/}).getAttribute('aria-checked'),'true');
await page.keyboard.press('Escape');
await page.getByRole('button',{name:'Canvas zoom',exact:true}).click();await page.getByRole('menu',{name:'Canvas zoom',exact:true}).waitFor();await page.waitForTimeout(1200);
if(!await page.getByRole('menu',{name:'Canvas zoom',exact:true}).count())failures.push('zoom dropdown vanished');
await page.getByRole('button',{name:'Canvas zoom',exact:true}).click();await page.waitForTimeout(150);
if(await page.getByRole('menu',{name:'Canvas zoom',exact:true}).count())failures.push('clicking dropdown trigger did not dismiss');
await page.keyboard.press('Escape');
const separator=page.locator('[data-bar-separator="pixen.document-frame"]');const before=await separator.getAttribute('data-bar-separator-before');
await separator.click({button:'right'});const menu=page.getByRole('menu',{name:'Status bar actions',exact:true});await menu.waitFor();
assert.equal(await menu.getByRole('menuitem',{name:'Move Earlier',exact:true}).count(),0);assert.equal(await menu.getByRole('menuitem',{name:'Move Later',exact:true}).count(),0);
const labels=await menu.locator('[role="menuitem"],[role="separator"]').evaluateAll(es=>es.map(e=>e.getAttribute('role')==='separator'?'---':e.textContent.trim()));assert.deepEqual(labels.slice(0,3),['Add Separator','Delete Separator','---']);await page.keyboard.press('Escape');
const box=await separator.boundingBox();const zoom=await page.locator('[data-status-id="pixen.document"]').boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(zoom.x,zoom.y+zoom.height/2,{steps:6});await page.mouse.up();assert.equal(await separator.getAttribute('data-bar-separator-before'),before);
assert.deepEqual(errors,[]);assert.deepEqual(failures,[]);console.log('PASS: status item menus survive updates; same-anchor clicks dismiss; preview speed remains open and selects; trigger click closes; separator grouping and locked drag pass');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exit(1)});
