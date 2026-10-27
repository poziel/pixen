const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');

(async () => {
  const browser = await chromium.launch({ channel: 'chrome', args: ['--no-proxy-server'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1100, height: 850 } });
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    let fixture = fs.readFileSync('scripts/fixtures/document-workspace.js', 'utf8');
    fixture = fixture.replace("export async function call(command,args={}){", `
      const connections = new Map(); let serial = 0;
      window.aiCalls = [];
      window.testArtwork = { create, activate: id => active = id, documents };
      export async function call(command,args={}){
        if (command.startsWith('ai_')) {
          window.aiCalls.push({ command, method: args.method, provider: args.provider, model: args.params?.model, references: args.params?.references });
          if (command === 'ai_drop_files') return [{name:'native.txt',data:btoa('Native brief')}];
          if (command === 'ai_test_provider' && window.failProviderTest) throw new Error('Unavailable');
          if (command === 'ai_models' || command === 'ai_test_provider') return [{id: args.provider + '-artist', label: 'Artist model'}];
          if (command === 'ai_connect') {
            if (window.failAiConnect) throw new Error('Provider setup is missing');
            const runtimeId = 'runtime-' + (++serial);
            connections.set(runtimeId, { documentId: args.documentId, events: args.events });
            return { runtimeId, cwd: '/artwork' };
          }
          if (command === 'ai_disconnect') { connections.delete(args.runtimeId); return; }
          const c = connections.get(args.runtimeId);
          if (!c) throw new Error('Closed runtime');
          if (args.method === 'session/create') return { sessionId: args.runtimeId, providerSessionId: args.runtimeId, provider: args.params.provider, capabilities: {} };
          if (args.method === 'turn/send') {
            const emit = event => c.events.onmessage({ sessionId: args.runtimeId, turnId: args.params.turnId, ...event });
            const doc = documents.find(d => d.id === c.documentId); doc.dirty = true; doc.generation++;
            emit({ type: 'activity', id: 'edit', label: 'Draw pixels', text: '', output: '', status: 'completed' });
            emit({ type: 'message', id: 'reply', text: 'Updated the artwork.\\n\\n**Result**\\n\\n| Pixels | Result |\\n| --- | --- |\\n| 4096 | OK |', phase: 'final', delta: false });
            if (!window.holdAiTurn) emit({ type: 'completed', status: 'completed' });
          }
          return null;
        }
    `);
    fixture = fixture.replace('activity:null', 'activity:{label:"AI drawing pixels",busy:true}');
    fixture += '\ncreate("a.pixloom");';
    await page.route('**/src/services/backend.ts', route => route.fulfill({ contentType: 'text/javascript', body: fixture }));
    await page.route('**/@tauri-apps_api_core.js*', route => route.fulfill({ contentType: 'text/javascript', body: 'export const isTauri=()=>true; export class Channel { onmessage=()=>{}; } export const invoke=async()=>null;' }));
    await page.route('**/@tauri-apps_api_window.js*', route => route.fulfill({ contentType: 'text/javascript', body: `export function getCurrentWindow(){return {
      isMaximized:async()=>false,isFullscreen:async()=>false,onResized:async()=>()=>{},
      onDragDropEvent:async callback=>{window.nativeDropListeners??=new Set();window.nativeDropListeners.add(callback);return ()=>window.nativeDropListeners.delete(callback)},onCloseRequested:async()=>()=>{},close:async()=>{},
      minimize:async()=>{},toggleMaximize:async()=>{},setFullscreen:async()=>{},startDragging:async()=>{}
    }}` }));
    await page.route('**/@tauri-apps_api_event.js*', route => route.fulfill({ contentType: 'text/javascript', body: 'export async function listen(){return ()=>{}}' }));
    await page.goto(process.env.PIXEN_TEST_URL || 'http://127.0.0.1:1464');
    await page.getByRole('button', { name: 'AI chat', exact: true }).click();
    const prompt = page.getByRole('textbox', { name: 'Message', exact: true });
    await prompt.waitFor();
    assert(await page.getByRole('button',{name:'Clear chat',exact:true}).evaluate(button=>!!button.closest('.section-header')));
    assert.equal(await page.locator('.art-chat-controls').count(),0);
    assert.deepEqual(await page.evaluate(() => window.aiCalls), [], 'Opening AI chat must not start providers');
    assert.equal(await page.getByLabel('AI provider', { exact: true }).count(), 0);
    assert.equal(await page.getByRole('button', {name:'Start conversation',exact:true}).count(), 0);
    async function settings(provider, model) {
      await page.keyboard.press('Control+,');
      const dialog = page.getByRole('dialog', {name:'Settings',exact:true});
      await dialog.waitFor();
      await dialog.getByRole('button', {name:'AI',exact:true}).click();
      await dialog.getByRole('combobox', {name:'AI provider',exact:true}).click();
      await page.getByRole('option', {name:({codex:'Codex',claude:'Claude',gemini:'Gemini',copilot:'GitHub Copilot',opencode:'OpenCode'})[provider],exact:true}).click();
      assert(await dialog.getByRole('combobox', {name:'AI model',exact:true}).isDisabled());
      await dialog.getByRole('button', {name:'Test provider',exact:true}).click();
      await dialog.getByText('Provider is available.', {exact:true}).waitFor();
      await dialog.getByRole('combobox', {name:'AI model',exact:true}).click();
      await page.getByRole('option', {name:'Artist model',exact:true}).click();
      if (provider === 'codex') await page.screenshot({path:'target/ai-settings-smoke.png'});
      await page.keyboard.press('Escape');
    }
    for (const provider of ['codex', 'claude', 'gemini', 'copilot', 'opencode']) {
      await settings(provider, provider + '-artist');
      await page.getByText('Describe what you want to create or change. Each message includes a fresh artwork preview. Attach images or text files for reference.',{exact:true}).waitFor();
      const before = await page.evaluate(() => window.aiCalls.filter(c => c.command === 'ai_connect').length);
      await prompt.fill('Draw a red pixel'); await prompt.press('Enter');
      await page.getByText('Updated the artwork.', { exact: true }).waitFor();
      assert.equal(await page.locator('.frame-chat-answer strong').innerText(), 'Result');
      assert.equal(await page.locator('.frame-chat-answer table tbody td').first().innerText(), '4096');
      assert.equal(await page.locator('.frame-chat-activity-group-summary').innerText(), 'Draw pixels (0s)');
      assert(!await page.locator('.status-bar').innerText().then(text => text.includes('AI drawing pixels')));

      assert.equal(await page.evaluate(() => window.aiCalls.filter(c => c.command === 'ai_connect').length), before + 1);
      const create = await page.evaluate(() => window.aiCalls.filter(c => c.method === 'session/create').at(-1));
      assert.equal(create.model, provider + '-artist');
      assert.equal(await page.getByRole('button', { name: 'Rewind to this message', exact: true }).count(), 0);
      await page.getByRole('button', { name: 'Clear chat', exact: true }).click();
      await prompt.waitFor();
      assert.equal(await page.evaluate(() => window.aiCalls.filter(c => c.command === 'ai_connect').length), before + 1, 'New conversation stays local until send');
    }
    assert.deepEqual(await page.evaluate(() => window.aiCalls.filter(c => c.command === 'ai_connect').map(c => c.provider)), ['codex','claude','gemini','copilot','opencode']);
    // Settings survive reload; no provider starts as a side effect.
    await page.reload();
    await page.getByRole('button', { name: 'AI chat', exact: true }).click();
    await prompt.waitFor();
    assert.deepEqual(await page.evaluate(() => window.aiCalls), []);
    await page.keyboard.press('Control+,');
    const dialog = page.getByRole('dialog', {name:'Settings',exact:true});
    await dialog.waitFor();
      await dialog.getByRole('button', {name:'AI',exact:true}).click();
    assert.match(await dialog.getByRole('combobox', {name:'AI provider',exact:true}).innerText(), /OpenCode/);
    assert(await dialog.getByRole('combobox', {name:'AI model',exact:true}).isDisabled());
    await page.evaluate(() => window.failProviderTest = true);
    await dialog.getByRole('button', {name:'Test provider',exact:true}).click();
    await dialog.getByRole('alert').filter({hasText:'This provider is not available.'}).waitFor();
    assert(await dialog.getByRole('combobox', {name:'AI model',exact:true}).isDisabled());
    await page.keyboard.press('Escape');
    await page.evaluate(() => window.failAiConnect = true);
    await prompt.fill('Retry this request'); await prompt.press('Enter');
    await page.getByText('Provider setup is missing', {exact:true}).waitFor();
    await page.evaluate(() => window.failAiConnect = false);
    await prompt.fill('Try again'); await prompt.press('Enter');
    await page.getByText('Updated the artwork.', {exact:true}).waitFor();
    await page.getByRole('button', {name:'Clear chat',exact:true}).click();
    await prompt.waitFor();
    const plus=page.getByRole('button', {name:'Add attachment',exact:true});
    await plus.click();
    const chooserEvent=page.waitForEvent('filechooser');
    await page.getByRole('menuitem', {name:'Files…',exact:true}).click();
    const chooser=await chooserEvent;
    await chooser.setFiles([{name:'reference.png',mimeType:'image/png',buffer:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==','base64')}]);
    // Drop a text brief alongside the selected reference, then send files without text.
    await page.locator('.frame-ai-prompt').evaluate(form => {
      const data=new DataTransfer(); data.items.add(new File(['Use warm colors'],'brief.txt',{type:'text/plain'}));
      form.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer:data}));
    });
    await page.locator('.art-chat').evaluate(async region => {
      const b=region.getBoundingClientRect(),factor=devicePixelRatio||1;
      await Promise.all([...window.nativeDropListeners].map(fn=>fn({payload:{type:'drop',paths:['C:/references/native.txt'],position:{x:(b.left+30)*factor,y:(b.top+100)*factor}}})));
    });
    await page.getByRole('button', {name:'Send message',exact:true}).click();
    await page.getByText('Updated the artwork.', {exact:true}).waitFor();
    const refs=await page.evaluate(() => window.aiCalls.filter(c => c.method==='turn/send').at(-1).references);
    assert.deepEqual(refs.map(r => r.name),['reference.png','brief.txt','native.txt']);
    assert.equal(Buffer.from(refs[1].data,'base64').toString(),'Use warm colors');
    await page.evaluate(() => window.holdAiTurn = true);
    await prompt.fill('Keep working'); await prompt.press('Enter');
    const border=await page.locator('.frame-ai-prompt').evaluate(form=>getComputedStyle(form,'::before').backgroundImage);
    assert(border.includes('204, 136, 0'),border);
    assert(!border.includes('0, 120, 212'),border);
    await page.getByRole('button', {name:'Stop response',exact:true}).click();
    assert(await page.evaluate(() => window.aiCalls.some(c => c.method === 'turn/cancel')));
    assert.deepEqual(errors, []);
    console.log('PASS: saved provider/model settings, first-message startup, five providers, file picker/browser and native drop, attachment-only send, retry and stop');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exit(1); });
