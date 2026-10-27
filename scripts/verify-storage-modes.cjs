const { chromium } = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
const {spawn,spawnSync} = require('node:child_process');
const assert = require('node:assert/strict');
const net = require('node:net');
const delay = ms => new Promise(r=>setTimeout(r,ms));
(async()=>{
 const root=path.resolve('target/storage-smoke-'+Date.now()); const app=path.join(root,'app');
 fs.mkdirSync(app,{recursive:true});
 const project=path.join(root,'original.pixen.json');
 fs.copyFileSync(path.resolve('skills/pixen/examples/static.pixloom.json'),project);
 const original=fs.readFileSync(project);
 for(const file of ['pixen.exe','pixen-mcp.exe']) fs.copyFileSync(path.resolve('target/release',file),path.join(app,file));
 const server=net.createServer(); await new Promise(r=>server.listen(0,'127.0.0.1',r));const port=server.address().port;await new Promise(r=>server.close(r));
 const env={...process.env,APPDATA:path.join(root,'roaming'),LOCALAPPDATA:path.join(root,'local'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port='+port};
 fs.mkdirSync(env.APPDATA,{recursive:true});fs.mkdirSync(env.LOCALAPPDATA,{recursive:true});
 const config=path.join(env.APPDATA,'com.poziel.pixen');fs.mkdirSync(config,{recursive:true});
 fs.writeFileSync(path.join(config,'settings.json'),JSON.stringify({'pixen.recoveryInterval':49}));
 fs.mkdirSync(path.join(app,'data/config'),{recursive:true});fs.writeFileSync(path.join(app,'data/config/conflict.txt'),'retain me');
 let browser,page;let mcp;
 const child=spawn(path.join(app,'pixen.exe'),[],{env,cwd:app,windowsHide:true,stdio:['ignore','ignore',fs.openSync(path.join(root,'app.log'),'a')]});
 child.on('exit',(code,signal)=>console.log('Initial app exit',code,signal));
 console.log('Test directory',root,'CDP',port,'PID',child.pid);
 async function attach(){
  const end=Date.now()+60000;
  while(Date.now()<end){try{const b=await chromium.connectOverCDP('http://127.0.0.1:'+port,{timeout:3000});const p=b.contexts()[0]?.pages().find(p=>!p.isClosed());if(p){await p.waitForFunction(()=>window.__TAURI_INTERNALS__&&document.querySelector('.workbench'),{},{timeout:5000});browser=b;page=p;return;}await b.close();}catch{}await delay(300);}
  throw Error('Unable to attach to restarted app: '+root);
 }
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
 async function migrate(mode){ console.log('Starting migration',mode);
  await page.keyboard.press('Control+,');
  const dialog=page.getByRole('dialog',{name:'Settings',exact:true});await dialog.waitFor();
  await dialog.getByRole('button',{name:'Installation',exact:true}).click();
  await dialog.getByRole('combobox',{name:'Storage mode',exact:true}).click();
  await page.getByRole('option',{name:mode==='semi-portable'?'Semi-portable':mode==='portable'?'Portable':'Default',exact:true}).click();
  await dialog.getByRole('button',{name:'Apply and restart',exact:true}).click();
  await page.waitForEvent('close',{timeout:20000});
  const end=Date.now()+70000;
  let completed;
  while(Date.now()<end){
   const dir=path.join(app,'.pixen-migrations');
   const dirs=fs.existsSync(dir)?fs.readdirSync(dir).map(x=>path.join(dir,x)):[];
   for(const d of dirs){if(fs.existsSync(path.join(d,'error.txt')))throw Error(fs.readFileSync(path.join(d,'error.txt'),'utf8'));}
   completed=dirs.filter(d=>fs.existsSync(path.join(d,'complete'))).length;
   if(completed===results.length+1)break;
   await delay(300);
  }
  assert.equal(completed,results.length+1,'migration helper did not complete');await attach();
  const status=await invoke('storage_status');assert.equal(status.mode,mode);
  const snapshot=await invoke('snapshot');assert.equal(snapshot.documents.length,2);assert(snapshot.documents.every(doc=>doc.dirty));assert(snapshot.documents.some(doc=>doc.path?.endsWith('original.pixen.json')));assert.deepEqual(fs.readFileSync(project),original);assert.equal(snapshot.document.width,13);assert.equal(snapshot.document.dirty,true);
  const settings=JSON.parse(await invoke('read_settings_file'));assert.equal(settings['pixen.recoveryInterval'],49);
  results.push(mode);console.log('PASS migration '+mode);
 }
 const results=[];
 try{
  await attach(); console.log('Attached'); await invoke('open_document',{path:project});await invoke('resize_document',{width:11,height:7});await invoke('create_document',{width:8,height:8});await invoke('resize_document',{width:13,height:9});
  mcp=spawn(path.join(app,'pixen-mcp.exe'),[],{env,cwd:app,windowsHide:true,stdio:['pipe','pipe','pipe']});await delay(500);
  await assert.rejects(()=>invoke('prepare_storage_migration',{target:'portable'}),/Close other Pixen/);console.log('MCP contention rejected',mcp.exitCode);mcp.stdin.end();if(mcp.exitCode===null)await new Promise(r=>mcp.once('exit',r));mcp=undefined;
  await migrate('portable');assert.equal(fs.readFileSync(path.join(app,'portable'),'utf8'),'');
  assert(fs.readdirSync(path.join(app,'data')).some(x=>x.startsWith('.pixen-backup-')&&fs.existsSync(path.join(app,'data',x,'conflict.txt'))));
  await migrate('semi-portable');assert.equal(fs.readFileSync(path.join(app,'portable'),'utf8').trim(),'semi-portable');
  await migrate('default');assert(!fs.existsSync(path.join(app,'portable')));
  const saved=(await invoke('snapshot')).documents.find(doc=>doc.path?.endsWith('original.pixen.json'));
  assert.equal(await invoke('save_document',{saveAs:false,documentId:saved.id}),true);
  assert.notDeepEqual(fs.readFileSync(project),original);
  const snap=await invoke('snapshot');for(const doc of snap.documents)await invoke('close_document',{documentId:doc.id,discard:true});
  await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('plugin:window|close',{label:'main'}));
  fs.writeFileSync(path.join(root,'result.json'),JSON.stringify({results,root},null,2));console.log('PASS isolated storage round trips, dirty artwork, JSON settings, backups and MCP contention: '+root);
 }finally{if(mcp)mcp.kill();if(browser)await browser.close().catch(()=>{});if(child.exitCode===null)child.kill();spawnSync('powershell',['-NoProfile','-Command',"Get-Process pixen -ErrorAction SilentlyContinue | Where-Object Path -eq $env:PIXEN_SMOKE_EXE | Stop-Process -Force"],{env:{...process.env,PIXEN_SMOKE_EXE:path.join(app,'pixen.exe')},windowsHide:true});}
})().catch(e=>{console.error(e);process.exitCode=1;});
