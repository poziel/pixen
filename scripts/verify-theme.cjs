// Start the Vite server first. Native transport is mocked; the real App and Frame render.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
(async () => {
  const browser = await chromium.launch({ headless: true, channel: process.env.PLAYWRIGHT_CHANNEL || 'msedge' });
  try {
    const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.route('**/src/services/backend.ts', route => route.fulfill({ contentType: 'text/javascript', body: fs.readFileSync('scripts/fixtures/document-workspace.js', 'utf8') }));
    await page.route('**/@tauri-apps_api_window.js*', route => route.fulfill({ contentType: 'text/javascript', body: 'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}' }));
    await page.route('**/@tauri-apps_api_event.js*', route => route.fulfill({ contentType: 'text/javascript', body: 'export async function listen(){return ()=>{}}' }));
    await page.goto(process.env.PIXLOOM_TEST_URL || 'http://127.0.0.1:1459');
    await page.waitForFunction(() => window.testCalls?.some(call => call.command === 'theme_config'));
    const tokens = await page.locator('.workbench').evaluate(element => {
      const style = getComputedStyle(element);
      return { accent: style.getPropertyValue('--frame-accent').trim(), body: style.fontSize, canvas: style.getPropertyValue('--frame-canvas').trim() };
    });
    assert.deepEqual(tokens, { accent: '#FE725E', body: '13px', canvas: '#151313' });
    await page.keyboard.press('Control+n');
    const width = page.getByLabel('Width', { exact: true });
    await width.waitFor();
    assert.equal(await width.evaluate(element => getComputedStyle(element).backgroundColor), 'rgb(39, 34, 32)');
    const primary = page.getByRole('button', { name: 'Create', exact: true });
    const colors = await primary.evaluate(element => ({ background: getComputedStyle(element).backgroundColor, foreground: getComputedStyle(element).color }));
    assert.deepEqual(colors, { background: 'rgb(254, 114, 94)', foreground: 'rgb(36, 23, 19)' });
    fs.mkdirSync('target/ui-verification', { recursive: true });
    await page.screenshot({ path: 'target/ui-verification/pixloom-theme-dialog.png' });
    await page.getByRole('button', { name: 'Cancel', exact: true }).click();
    await page.keyboard.press('Control+Shift+p');
    const input = page.getByPlaceholder(/Type a Frame command/);
    await input.waitFor();
    const outline = await input.evaluate(element => ({ width: getComputedStyle(element).outlineWidth, offset: getComputedStyle(element).outlineOffset, color: getComputedStyle(element).outlineColor }));
    assert.deepEqual(outline, { width: '1px', offset: '-2px', color: 'rgb(254, 114, 94)' });
    assert.equal(await page.locator('.command-result.active').evaluate(element => getComputedStyle(element).color), 'rgb(36, 23, 19)');
    await page.screenshot({ path: 'target/ui-verification/pixloom-theme-palette.png' });
    assert.deepEqual(errors, []);
    console.log('Verified native YAML import, warm palette, logo accent, dialog controls, and inset focus ring.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
