const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try {
  for(const mode of ['normal','portable']){
   const directory=fs.mkdtempSync(path.resolve('target/settings-browser-'+mode+'-'));
   const file=path.join(directory,mode==='portable'?'application/data/config/settings.json':'user-appdata/com.poziel.pixen/settings.json');
   let holdWrites=false, releaseWrite;
   const context=await browser.newContext({viewport:{width:1200,height:850}});
   const page=await context.newPage();page.setDefaultTimeout(15000);const errors=[];
   page.on('pageerror',error=>errors.push(error.message));
   await page.addInitScript(()=>{
    if(localStorage.getItem('migration-fixture'))return;
    localStorage.setItem('migration-fixture','true');
    localStorage.setItem('pixen.recoveryInterval','47');
    localStorage.setItem('pixen.ai-settings.v1',JSON.stringify({provider:'claude',models:{codex:'codex-saved',claude:'claude-saved'}}));
    localStorage.setItem('pixen.theme','custom');
   });
   await page.route('**/__settings',async route=>{
    if(route.request().method()==='POST'){
     if(holdWrites)await new Promise(resolve=>{releaseWrite=resolve});
     fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,route.request().postData());
     await route.fulfill({contentType:'application/json',body:'null'});
    }else await route.fulfill({contentType:'application/json',body:JSON.stringify(fs.existsSync(file)?fs.readFileSync(file,'utf8'):null)});
   });
   let fixture=fs.readFileSync('scripts/fixtures/document-workspace.js','utf8');
   fixture=fixture.replace('export async function call(command,args={}){',`export async function call(command,args={}){
    if(command==='read_settings_file')return fetch('/__settings').then(r=>r.json());
    if(command==='write_settings_file'){const r=await fetch('/__settings',{method:'POST',body:args.json});if(!r.ok)throw Error('Settings write failed');return;}
    if(command==='configure_recovery'){window.recoveryInterval=args.seconds;return;}
    if(command==='desktop_ready')return;
    if(command==='theme_config')return {files:[{file_name:'custom.json',source:{format:'json',text:JSON.stringify({version:1,id:'custom',name:'Custom theme',appearance:'dark',tokens:{'frame.accent':'#123456'}})}}]};
    if(command==='ai_test_provider')return [{id:args.provider+'-saved',label:'Saved model'},{id:args.provider+'-new',label:'New model'}];
   `);
   await page.route('**/src/services/backend.ts',route=>route.fulfill({contentType:'text/javascript',body:fixture}));
   await page.route('**/@tauri-apps_api_window.js*',route=>route.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async callback=>{window.closeCallback=callback;return ()=>{}},close:async()=>{window.closeCalls=(window.closeCalls||0)+1}}}'}));
   await page.route('**/@tauri-apps_api_event.js*',route=>route.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
   const firstWrite=page.waitForResponse(r=>r.url().endsWith('/__settings')&&r.request().method()==='POST');
   await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1422');await firstWrite;
   let saved=JSON.parse(fs.readFileSync(file,'utf8'));
   assert.equal(saved['pixen.recoveryInterval'],47);assert.equal(saved['pixen.ai.provider'],'claude');
   assert.equal(saved['pixen.ai.models.codex'],'codex-saved');assert.equal(saved['pixen.ai.models.claude'],'claude-saved');assert.equal(saved['pixen.theme'],'custom');
   await page.keyboard.press('Control+,');
   const dialog=page.getByRole('dialog',{name:'Settings',exact:true});await dialog.waitFor();
   await dialog.getByRole('button',{name:'AI',exact:true}).click();
   const model=dialog.getByRole('combobox',{name:'Model',exact:true});assert(await model.isDisabled());assert.match(await model.innerText(),/claude-saved/);
   await dialog.getByRole('button',{name:'Test provider',exact:true}).click();await dialog.getByText('Provider is available.',{exact:true}).waitFor();
   await model.click();const nextWrite=page.waitForResponse(r=>r.url().endsWith('/__settings')&&r.request().method()==='POST');
   await page.getByRole('option',{name:'New model',exact:true}).click();await nextWrite;
   await dialog.getByRole('button',{name:'Recovery',exact:true}).click();
   const interval=dialog.getByRole('spinbutton',{name:'Save recovery file every (seconds)',exact:true});
   assert.equal(await interval.inputValue(),'47');
   const changed=page.waitForResponse(r=>r.url().endsWith('/__settings')&&r.request().method()==='POST');
   await interval.fill('62');await interval.press('Tab');await changed;
   await page.evaluate(()=>localStorage.setItem('pixen.recoveryInterval','999'));
   await page.reload();await page.locator('.workbench').waitFor();await page.waitForFunction(()=>window.recoveryInterval===62);
   await page.keyboard.press('Control+,');await dialog.waitFor();
   await dialog.getByRole('button',{name:'AI',exact:true}).click();assert(await model.isDisabled());assert.match(await model.innerText(),/claude-new/);
   saved=JSON.parse(fs.readFileSync(file,'utf8'));assert.equal(saved['pixen.recoveryInterval'],62);assert.equal(saved['pixen.ai.models.codex'],'codex-saved');
   assert.equal(await dialog.evaluate(el=>getComputedStyle(el).getPropertyValue('--frame-accent').trim().toLowerCase()),'#123456');
   await dialog.getByRole('button',{name:'Recovery',exact:true}).click();
   holdWrites=true;
   const pending=page.waitForRequest(r=>r.url().endsWith('/__settings')&&r.method()==='POST');
   await interval.fill('63');await interval.press('Tab');await pending;
   await page.evaluate(()=>{window.closePromise=window.closeCallback({preventDefault(){window.closePrevented=true}})});
   await page.waitForFunction(()=>window.closePrevented);
   assert.equal(await page.evaluate(()=>window.closeCalls||0),0,'Closing must wait for pending settings writes');
   holdWrites=false;releaseWrite();await page.evaluate(()=>window.closePromise);
   assert.equal(await page.evaluate(()=>window.closeCalls),1);
   assert.equal(JSON.parse(fs.readFileSync(file,'utf8'))['pixen.recoveryInterval'],63);
   assert.deepEqual(errors,[]);console.log(mode+': migration, saved edits, reload, and waiting for file writes before close passed.');
   fs.writeFileSync(file,'{broken');
   await page.reload();await page.locator('.workbench').waitFor();
   await page.waitForFunction(()=>typeof window.closeCallback==='function');
   await page.evaluate(()=>window.closeCallback({preventDefault(){}}));
   assert.equal(await page.evaluate(()=>window.closeCalls),1);
   assert.equal(fs.readFileSync(file,'utf8'),'{broken');
   assert.deepEqual(errors,[]);
   console.log(mode+': malformed file remains unchanged and does not block closing.');
   await context.close();
  }
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1});
