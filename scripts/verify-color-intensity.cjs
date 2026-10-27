const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try{
  const page=await browser.newPage({viewport:{width:1100,height:1000}}),errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.route('**/src/services/backend.ts',route=>route.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/manual-tools.js','utf8')}));
  await page.route('**/@tauri-apps_api_window.js*',route=>route.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',route=>route.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1459');
  await page.getByRole('button',{name:/Open artwork/}).click();
  if (!await page.locator('.color-picker').isVisible()) await page.getByRole('button',{name:'Toggle primary sidebar',exact:true}).click();
  await page.getByRole('button',{name:'Tools',exact:true}).click();
  for(const name of [/^Left tools/,/^Right tools/]){
   const button=page.getByRole('button',{name,exact:true});if(await button.getAttribute('aria-expanded')==='true')await button.click();
  }

  const panel=page.locator('.advanced-color-options'),hex=page.getByLabel('Hex color, optional alpha');
  async function number(name,value){const input=page.getByRole('spinbutton',{name:name+' value',exact:true});await input.fill(String(value));await input.press('Tab');}
  async function setHex(value){await hex.fill(value);await hex.press('Tab');}
  async function mode(name){await panel.getByRole('tab',{name,exact:true}).click();}
  const value=name=>page.getByRole('spinbutton',{name:name+' value',exact:true}).inputValue();
  async function slider(name,ratio){const track=panel.locator('.color-channel-row').filter({has:page.getByRole('spinbutton',{name:name+' value',exact:true})}).locator('.frame-color-track,.frame-slider-track');const box=await track.boundingBox();await page.mouse.click(box.x+box.width*ratio,box.y+box.height/2);await page.waitForTimeout(80);}
  await setHex('80402070');
  const original=await hex.inputValue();
  const rgbValues=await Promise.all(['Red','Green','Blue','Alpha'].map(value));
  const area=page.locator('.frame-hsv-area');
  const cursor=()=>area.locator('.frame-hsv-plane-root [role=slider]').first().evaluate(el=>el.getAttribute('style'));
  const cursorBefore=await cursor();
  await number('Intensity',2);
  assert.notEqual(await hex.inputValue(),original);
  assert.deepEqual(await Promise.all(['Red','Green','Blue','Alpha'].map(value)),rgbValues);
  assert.equal(await cursor(),cursorBefore,'exposure must not move the base picker cursor');
  await mode('HSV');const baseHsv=await Promise.all(['Hue','Saturation','Value'].map(value));
  await number('Intensity',-2);
  assert.deepEqual(await Promise.all(['Hue','Saturation','Value'].map(value)),baseHsv);
  await number('Hue',210);assert.equal(Number(await value('Intensity')),-2);
  await mode('RGB');await number('Red',160);assert.equal(Number(await value('Intensity')),-2);
  const base=await Promise.all(['Red','Green','Blue'].map(value));
  const dark=await hex.inputValue();await number('Intensity',0);
  const expected=base.map(n=>Number(n).toString(16).padStart(2,'0')).join('')+'70';
  assert.equal(await hex.inputValue(),expected);
  assert.notEqual(dark,expected);
  await number('Intensity',2);
  const plane=area.locator('.frame-hsv-plane-root .frame-color-plane'),box=await plane.boundingBox();
  await page.mouse.click(box.x+box.width*.4,box.y+box.height*.55);
  assert.equal(Number(await value('Intensity')),2,'picker editing must preserve exposure');
  const picked=await Promise.all(['Red','Green','Blue'].map(value));
  const final=await hex.inputValue();await number('Intensity',0);
  assert.equal(await hex.inputValue(),picked.map(n=>Number(n).toString(16).padStart(2,'0')).join('')+'70');
  await number('Intensity',2);assert.equal(await hex.inputValue(),final);
  await page.getByRole('button',{name:/^secondary color/}).click();assert.equal(Number(await value('Intensity')),0);
  await number('Intensity',-1);
  await page.getByRole('button',{name:/^primary color/}).click();assert.equal(Number(await value('Intensity')),2);assert.equal(await hex.inputValue(),final);
  await mode('Linear');assert.equal(Number(await value('Intensity')),2);
  await number('Red',.2);assert.equal(Number(await value('Intensity')),2);
  await mode('RGB');
  // Track endpoints and the dragged thumb should line up with the actual pointer.
  const row=panel.locator('[data-channel=R]'),track=row.locator('.frame-color-track');
  await track.scrollIntoViewIfNeeded();const trackBox=await track.boundingBox();
  const thumb=row.locator('.frame-color-thumb');const thumbBox=await thumb.boundingBox();
  await page.mouse.move(thumbBox.x+thumbBox.width/2,thumbBox.y+thumbBox.height/2);await page.mouse.down();
  for(const ratio of [.2,.5,.8]){
    const x=trackBox.x+trackBox.width*ratio,y=trackBox.y+trackBox.height/2;
    await page.mouse.move(x,y,{steps:5});
    const marker=await thumb.boundingBox();
    assert(Math.abs(marker.x+marker.width/2-x)<2,'thumb must track pointer without a horizontal gap');
    assert(Math.abs(marker.y+marker.height/2-y)<2,'thumb must stay vertically centered: '+JSON.stringify({marker,trackBox,y}));
  }
  await page.mouse.up();
  const paintHex=await hex.inputValue();
  const art=page.locator('canvas.artwork:not(.selection-overlay)');await page.waitForFunction(()=>document.querySelector('canvas.artwork')?.width===16);
  const artBox=await art.boundingBox();await page.mouse.click(artBox.x+artBox.width*.4,artBox.y+artBox.height*.4);await page.waitForTimeout(200);
  const ink=await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').at(-1)?.args.pixels[0].color);
  assert.equal(ink,'#'+paintHex,'painting uses the intensity-adjusted output');
  fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/color-intensity.png'});
  assert.deepEqual(errors,[]);
  console.log('PASS: independent base color and intensity, stable channels/cursor, persistent exposure during picker/channel edits, isolated targets, accurate thumb tracking, and final paint color.');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exit(1)});
