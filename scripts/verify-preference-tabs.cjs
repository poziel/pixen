const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try{
 for(const mode of ['normal','portable']){
 const context=await browser.newContext({viewport:{width:1280,height:900},permissions:['clipboard-read','clipboard-write']});
 const page=await context.newPage();page.setDefaultTimeout(15000);const errors=[];page.on('pageerror',e=>errors.push(e.message));
 let settings='{}',keybindings='{}';
 const path=require('node:path');const directory=fs.mkdtempSync(path.resolve('target/preference-tabs-'+mode+'-'));const config=path.join(directory,mode==='portable'?'application/data/config':'user-appdata/com.poziel.pixen');
 fs.mkdirSync(config,{recursive:true});
 await page.route('**/__preferences/*',async route=>{const kind=route.request().url().split('/').at(-1);if(route.request().method()==='POST'){if(kind==='settings')settings=route.request().postData();else keybindings=route.request().postData();fs.writeFileSync(path.join(config,kind+'.json'),kind==='settings'?settings:keybindings);await route.fulfill({body:'null',contentType:'application/json'});}else await route.fulfill({body:JSON.stringify(kind==='settings'?settings:keybindings),contentType:'application/json'});});
 const fixture=fs.readFileSync('scripts/fixtures/document-workspace.js','utf8').replace('export async function call(command,args={}){',`export async function call(command,args={}){
 window.testBackend=call;
 if(command==='read_settings_file'||command==='read_keybindings_file')return fetch('/__preferences/'+(command==='read_settings_file'?'settings':'keybindings')).then(r=>r.json());
 if(command==='write_settings_file'||command==='write_keybindings_file'){await fetch('/__preferences/'+(command==='write_settings_file'?'settings':'keybindings'),{method:'POST',body:args.json});return;}
 `);
 await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fixture}));
 await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async cb=>{window.closeCallback=cb;return ()=>{}},close:async()=>{window.closedOK=true}}}'}));
 await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
 async function attach(){await page.locator('.workbench').waitFor();await page.evaluate(()=>{window.testFrame=document.querySelector('.workbench').__vueParentComponent.props.frame});}
 async function selected(id){await page.waitForFunction(id=>testFrame.workspace.activeTabs.value[testFrame.workspace.initialPaneId]===id,id);await page.waitForTimeout(1050);assert.equal(await page.evaluate(()=>testFrame.workspace.activeTabs.value[testFrame.workspace.initialPaneId]),id);}
 await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1422');await attach();
 assert.equal(await page.getByRole('tab').count(),0);
 for(const kind of ['settings','keybindings']){
 const label=kind==='settings'?'Settings':'Keyboard Shortcuts';
 await page.evaluate(kind=>kind==='settings'?testFrame.openSettings():testFrame.openKeybindings(),kind);
 let modal=page.getByRole('dialog',{name:label,exact:true});await modal.waitFor();
 await modal.getByRole('searchbox',{name:'Search '+label.toLowerCase()}).fill(kind==='settings'?'recovery':'pixen.open');
 await modal.getByRole('button',{name:`Open ${label} in Editor Tab`,exact:true}).click();await selected('frame.'+kind);
 assert.equal(await page.getByRole('dialog').count(),0);assert(await page.getByRole('tab',{name:label,exact:true}).isVisible());
 const surface=page.locator(`[data-frame-${kind}-surface]:visible`);
 assert.equal(await surface.getByRole('searchbox').inputValue(),kind==='settings'?'recovery':'pixen.open');
 await surface.getByRole('button',{name:`Open ${label} in Modal`,exact:true}).click();await modal.waitFor();
 await modal.getByRole('button',{name:'Maximize modal',exact:true}).click();
 await modal.getByRole('button',{name:`Open ${label} (JSON)`,exact:true}).click();
 modal=page.getByRole('dialog',{name:kind+'.json',exact:true});await modal.waitFor();assert(await modal.evaluate(e=>e.classList.contains('maximized')));
 await modal.locator('.monaco-editor').waitFor();await page.waitForFunction(()=>document.querySelector('.frame-code-editor')?.getAttribute('aria-busy')==='false');
 const json=kind==='settings'?'{"pixen.recoveryInterval":61,"pixen.ai.models.codex":"json-model"}':'{"pixen.open":"Ctrl+Alt+O","pixen.new":null}';
 await modal.locator('.monaco-editor .view-lines').click({position:{x:40,y:10}});await page.keyboard.press('Control+a');await page.evaluate(text=>navigator.clipboard.writeText(text),json);await page.keyboard.press('Control+v');
 await page.waitForFunction(kind=>testFrame[kind].json.dirty.value,kind);
 await modal.getByRole('button',{name:`Open ${label} in Editor Tab`,exact:true}).click();await selected('frame.'+kind);
 assert.equal(await page.evaluate(kind=>testFrame[kind].json.draft.value,kind),json);
 await page.locator(`[data-frame-${kind}-surface]:visible .monaco-editor .view-lines`).click({position:{x:40,y:10}});await page.keyboard.press('Control+s');
 await page.waitForFunction(kind=>!testFrame[kind].json.dirty.value,kind).catch(async error=>{console.log(await page.evaluate(kind=>({kind,error:testFrame[kind].json.error.value,saving:testFrame[kind].json.saving.value,effective:testFrame[kind].toJSON(),draft:testFrame[kind].json.draft.value,focused:document.activeElement?.outerHTML.slice(0,180),calls:window.testCalls?.slice(-5)}),kind));await page.screenshot({path:'target/preferences-failure.png'});throw error});
 assert.deepEqual(JSON.parse(fs.readFileSync(path.join(config,kind+'.json'),'utf8')),JSON.parse(json));
 await page.locator(`[data-frame-${kind}-surface]:visible`).getByRole('button',{name:`Open ${label} (UI)`,exact:true}).click();
 }
 // Both pages remain selectable across backend polling with multiple documents.
 await page.evaluate(()=>testBackend('open_document',{path:'a.pixloom'}));await selected('document-1');
 await page.evaluate(()=>testBackend('open_document',{path:'b.pixloom'}));await selected('document-2');
 for(const name of ['Settings','a.pixloom','Keyboard Shortcuts','b.pixloom','Settings']){await page.getByRole('tab',{name,exact:true}).click();await selected(name==='Settings'?'frame.settings':name==='Keyboard Shortcuts'?'frame.keybindings':name==='a.pixloom'?'document-1':'document-2');}
 await page.evaluate(()=>testFrame.workspace.closeTabs(['document-1','document-2']));await selected('frame.settings');
 assert.deepEqual(await page.evaluate(()=>testFrame.workspace.tabs.value.map(t=>t.id).sort()),['frame.keybindings','frame.settings']);
 await page.reload();await attach();
 assert.equal(await page.evaluate(()=>testFrame.settings.read('pixen.recoveryInterval')),61);
 assert.equal(await page.evaluate(()=>testFrame.settings.read('pixen.ai.models.codex')),'json-model');
 assert.equal(await page.evaluate(()=>testFrame.commands.get('pixen.open').shortcut),'Ctrl+Alt+O');
 assert.equal(await page.evaluate(()=>testFrame.commands.get('pixen.new').shortcut),undefined);
 assert.deepEqual(errors,[]);await page.screenshot({path:`target/preferences-${mode}.png`});
 console.log(mode+': empty workspace, shared search/header, modal bounds, JSON drafts/move/save/reload, artwork polling and document closing passed.');
 await context.close();
 }
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1});
