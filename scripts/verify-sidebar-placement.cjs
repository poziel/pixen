const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
(async () => {
  const browser = await chromium.launch({ headless: true, channel: process.env.PLAYWRIGHT_CHANNEL || 'msedge' });
  try {
    for (const initiallyLoaded of [false, true]) {
      const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
      page.setDefaultTimeout(10000);
      const errors = []; page.on('pageerror', error => { errors.push(error.message); console.error(error.message); });
      let backend = fs.readFileSync('scripts/fixtures/document-workspace.js', 'utf8');
      if (initiallyLoaded) backend = backend.replace('export async function call', "create('startup.pixloom');\nexport async function call");
      await page.route('**/src/services/backend.ts', route => route.fulfill({ contentType: 'text/javascript', body: backend }));
      await page.route('**/@tauri-apps_api_window.js*', route => route.fulfill({ contentType: 'text/javascript', body: 'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}' }));
      await page.route('**/@tauri-apps_api_event.js*', route => route.fulfill({ contentType: 'text/javascript', body: 'export async function listen(){return ()=>{}}' }));
      await page.goto(process.env.PIXEN_TEST_URL || 'http://127.0.0.1:1459');
      await page.waitForFunction(() => window.testCalls?.some(call => call.command === 'desktop_ready'));
      const toggle = page.locator('.status-left').getByRole('button', { name: 'Toggle primary sidebar', exact: true });
      await toggle.waitFor();
      assert.equal(await page.locator('.titlebar').getByRole('button', { name: 'Toggle primary sidebar', exact: true }).count(), 0);
      assert.equal(await toggle.getAttribute('aria-expanded'), String(initiallyLoaded));
      const beforeResolution = await toggle.evaluate(element => {
        const resolution = document.querySelector('[data-status-id="pixen.document"]');
        return element.getBoundingClientRect().right <= resolution.getBoundingClientRect().left;
      });
      assert.equal(beforeResolution, true);
      await toggle.click(); assert.equal(await toggle.getAttribute('aria-expanded'), String(!initiallyLoaded));
      await page.keyboard.press('Control+b');
      assert.equal(await toggle.getAttribute('aria-expanded'), String(initiallyLoaded), 'existing shortcut uses the same control');
      await page.keyboard.press('Control+b');
      await page.waitForFunction(() => window.testCalls.filter(call => call.command === 'snapshot').length >= 3);
      assert.equal(await toggle.getAttribute('aria-expanded'), String(!initiallyLoaded), 'polling preserves choice');
      if (!initiallyLoaded) {
        // Closing the final document collapses the sidebar through either close path.
        await page.keyboard.press('Control+o');
        await page.getByText('32 × 32', { exact: true }).waitFor();
        assert.equal(await toggle.getAttribute('aria-expanded'), 'true');
        await page.keyboard.press('Control+w');
        await page.getByText('No artwork', { exact: true }).waitFor();
        assert.equal(await toggle.getAttribute('aria-expanded'), 'false');
        await page.keyboard.press('Control+o');
        await page.getByText('32 × 32', { exact: true }).waitFor();
        await toggle.click();
        await page.getByRole('menuitem', { name: 'File', exact: true }).click();
        await page.getByRole('menuitem', { name: /^Close artwork/ }).click();
        await page.getByText('No artwork', { exact: true }).waitFor();
        assert.equal(await toggle.getAttribute('aria-expanded'), 'false');
        fs.mkdirSync('target/ui-verification', { recursive: true });
        await page.screenshot({ path: 'target/ui-verification/sidebar-empty-start.png' });
      } else {
        await toggle.click();
        await page.keyboard.press('Control+o');
        await page.locator('.titlebar-text').filter({ hasText: 'a.pixloom' }).waitFor();
        await page.keyboard.press('Control+w');
        await page.locator('.titlebar-text').filter({ hasText: 'startup.pixloom' }).waitFor();
        assert.equal(await toggle.getAttribute('aria-expanded'), 'true', 'another loaded document keeps sidebar open');
        await page.keyboard.press('Control+w');
        await page.getByText('No artwork', { exact: true }).waitFor();
        assert.equal(await toggle.getAttribute('aria-expanded'), 'false');
      }
      assert.deepEqual(errors, []); await page.close();
    }
    console.log('Verified empty/loaded startup, status toggle, and sidebar collapse after Ctrl+W and File-menu closing.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
