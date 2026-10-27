const {chromium}=require('playwright');
const fs=require('node:fs');const assert=require('node:assert/strict');
(async()=>{const browser=await chromium.launch({channel:'chrome',headless:true});try{
 const page=await browser.newPage({viewport:{width:1280,height:900}});page.setDefaultTimeout(15000);const errors=[];page.on('pageerror',e=>errors.push(e.message));
 let fixture=fs.readFileSync('scripts/fixtures/document-workspace.js','utf8');
 fixture=fixture.replace('export async function call(command,args={}){',`export async function call(command,args={}){
  if(command==='read_settings_file')return sessionStorage.getItem('native-settings')??JSON.stringify({'pixen.background':'black','pixen.fps':'8','pixen.representation':'grayscale'});
  if(command==='write_settings_file'){sessionStorage.setItem('native-settings',args.json);return;}
  if(command==='configure_recovery')return;
  if(command==='open_document'&&window.nextOpenArtwork)args={...args,path:window.nextOpenArtwork};
 `);fixture+='\ncreate("initial-animation.pixloom");';
 await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fixture}));
 await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async handler=>{window.dropArtwork=handler;return ()=>{}},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
 await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
 await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1422');
 const canvas=page.locator('canvas.artwork:not(.selection-overlay)');
 const fps=page.locator('[data-status-id="pixen.fps"]');
 await page.locator('canvas.artwork.background-black').waitFor();assert.match(await fps.innerText(),/8 FPS/);
 async function defaults(background,speed){
  await page.keyboard.press('Control+,');const dialog=page.getByRole('dialog',{name:'Settings',exact:true});await dialog.waitFor();
  await dialog.getByRole('button',{name:'Canvas',exact:true}).click();
  assert.equal(await dialog.getByRole('combobox',{name:'Color mode',exact:true}).count(),0);
  await dialog.getByRole('combobox',{name:'Default background',exact:true}).click();
  await page.getByRole('option',{name:background,exact:true}).click();
  await dialog.getByRole('button',{name:'Playback',exact:true}).click();
  await dialog.getByRole('combobox',{name:'Default preview speed',exact:true}).click();
  await page.getByRole('option',{name:speed+' FPS',exact:true}).click();
  await dialog.getByRole('button',{name:'Close modal',exact:true}).click();await dialog.waitFor({state:'hidden'});
 }
 await defaults('White',15);
 assert((await canvas.getAttribute('class')).includes('background-black'));assert.match(await fps.innerText(),/8 FPS/);
 await page.waitForFunction(()=>typeof window.dropArtwork==='function');
 await page.evaluate(()=>window.nextOpenArtwork='second-animation.pixloom');await page.keyboard.press('Control+o');
 await page.locator('canvas.artwork.background-white').waitFor();assert.match(await fps.innerText(),/15 FPS/);
 await page.getByRole('menuitem',{name:'View',exact:true}).click();
 await page.getByRole('menuitem',{name:'Background',exact:true}).hover();
 await page.getByRole('menuitemcheckbox',{name:'transparent',exact:true}).click().catch(async error=>{console.log(await page.locator('[role^=menuitem]').evaluateAll(items=>items.map(el=>[el.getAttribute('role'),el.textContent])));await page.screenshot({path:'target/defaults-failure.png'});throw error});
 await page.locator('canvas.artwork.background-transparent').waitFor();
 await page.locator('.canvas-stage').focus();await page.keyboard.press('Control+F7');
 await page.waitForFunction(()=>document.querySelector('[data-status-id="pixen.fps"]')?.textContent.includes('16 FPS'));
 assert.equal(JSON.parse(await page.evaluate(()=>sessionStorage.getItem('native-settings')))['pixen.fps'],'15');
 assert.equal(JSON.parse(await page.evaluate(()=>sessionStorage.getItem('native-settings')))['pixen.background'],'white');
 await defaults('Black',5);
 assert((await canvas.getAttribute('class')).includes('background-transparent'));assert.match(await fps.innerText(),/16 FPS/);
 await page.evaluate(()=>window.nextOpenArtwork='third-animation.pixloom');await page.keyboard.press('Control+o');
 await page.locator('canvas.artwork.background-black').waitFor();assert.match(await fps.innerText(),/5 FPS/);
 await page.getByRole('tab',{name:'second-animation.pixloom',exact:true}).click();
 await page.locator('canvas.artwork.background-transparent').waitFor();assert.match(await fps.innerText(),/16 FPS/);
 await page.getByRole('tab',{name:'initial-animation.pixloom',exact:true}).click();
 await page.locator('canvas.artwork.background-black').waitFor();assert.match(await fps.innerText(),/8 FPS/);
 const saved=JSON.parse(await page.evaluate(()=>sessionStorage.getItem('native-settings')));
 assert.equal(saved['pixen.background'],'black');assert.equal(saved['pixen.fps'],'5');assert.equal(saved['pixen.representation'],undefined);
 await page.keyboard.press('Control+n');const dialog=page.getByRole('dialog',{name:'New artwork',exact:true});await dialog.waitFor();await dialog.getByRole('button',{name:'Create',exact:true}).click();
 await dialog.waitFor({state:'hidden'});await page.locator('canvas.artwork.background-black').waitFor();
 assert.deepEqual(errors,[]);console.log('PASS: stored startup defaults, non-mutating settings, independent artwork overrides, tab restoration, new-artwork defaults, and retired global color mode.');
}finally{await browser.close();}})().catch(e=>{console.error(e);process.exitCode=1});
