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

  assert.equal(await page.locator('details.color-options').count(),0);
  const panel=page.locator('.advanced-color-options'),hex=page.getByLabel('Hex color, optional alpha');
  async function number(name,value){const input=page.getByRole('spinbutton',{name:name+' value',exact:true});await input.fill(String(value));await input.press('Tab');}
  async function setHex(value){await hex.fill(value);await hex.press('Tab');}
  async function mode(name){await panel.getByRole('tab',{name,exact:true}).click();}
  const value=name=>page.getByRole('spinbutton',{name:name+' value',exact:true}).inputValue();
  async function slider(name,ratio){const track=panel.locator('.color-channel-row').filter({has:page.getByRole('spinbutton',{name:name+' value',exact:true})}).locator('.frame-color-track,.frame-slider-track');const box=await track.boundingBox();await page.mouse.click(box.x+box.width*ratio,box.y+box.height/2);await page.waitForTimeout(80);}
  const selectedTab=panel.getByRole('tab',{selected:true});
  const tabStyle=await selectedTab.evaluate(el=>{const s=getComputedStyle(el);return {background:s.backgroundColor,radius:s.borderRadius,indicator:s.borderBottomWidth};});
  assert.equal(tabStyle.background,'rgba(0, 0, 0, 0)');assert.equal(tabStyle.radius,'0px');assert.equal(tabStyle.indicator,'2px');
  const compact=panel.locator('.frame-number-field').first();
  await compact.getByRole('spinbutton').focus();
  assert.equal(await compact.getAttribute('data-size'),'compact');
  assert.equal(await compact.locator('button').count(),0);assert.equal(await compact.getByRole('spinbutton').getAttribute('type'),'number');
  const focusStyle=await compact.evaluate(el=>{const style=getComputedStyle(el);const probe=document.createElement('span');probe.style.color='var(--frame-accent)';el.append(probe);const accent=getComputedStyle(probe).color;probe.remove();return {outline:style.outlineColor,accent,inputOutline:getComputedStyle(el.querySelector('input')).outlineStyle};});
  assert.equal(focusStyle.outline,focusStyle.accent);assert.equal(focusStyle.inputOutline,'none');
  const pieces=await compact.locator('button,input').evaluateAll(nodes=>nodes.map(el=>el.getBoundingClientRect().toJSON()));
  assert.equal(await compact.getAttribute('data-controls'),'stacked');
  assert.equal(pieces.length,1,'stacked mode uses the native number input without extra button padding');
  await hex.focus();await page.getByRole('tab',{name:'RGB',exact:true}).focus();
  const surface=el=>{const css=getComputedStyle(el);return [css.borderColor,css.borderRadius,css.borderWidth];};
  assert.deepEqual(await compact.evaluate(surface),await hex.evaluate(surface),'Frame number and text input default borders match');
  await compact.getByRole('spinbutton').focus();const numberFocus=await compact.evaluate(el=>[getComputedStyle(el).outlineColor,getComputedStyle(el).outlineStyle,getComputedStyle(el).borderColor]);
  await hex.focus();assert.deepEqual(await hex.evaluate(el=>[getComputedStyle(el).outlineColor,getComputedStyle(el).outlineStyle,getComputedStyle(el).borderColor]),numberFocus,'Frame number and text input focus matches');
  await setHex('80402070');
  assert.equal(await panel.locator('.frame-color-slider').count(),4);
  assert.equal(await panel.locator('.frame-number-field').count(),5);
  assert.equal(await value('Red'),'128');assert.equal(await value('Alpha'),'112');
  await page.getByRole('spinbutton',{name:'Red value',exact:true}).fill('200');assert((await hex.inputValue()).startsWith('c84020'),'number input previews before blur');
  await number('Red',200);assert((await hex.inputValue()).startsWith('c84020'));
  const gradientBefore=await panel.locator('[data-channel=G] .frame-color-track').evaluate(el=>getComputedStyle(el).backgroundImage);
  await slider('Red',.5);const red=Number(await value('Red'));assert(Math.abs(red-128)<4,`red=${red},hex=${await hex.inputValue()}`);
  assert.notEqual(await panel.locator('[data-channel=G] .frame-color-track').evaluate(el=>getComputedStyle(el).backgroundImage),gradientBefore);
  assert.equal(await value('Alpha'),'112');
  const dragRow=panel.locator('[data-channel=R]'),dragTrack=await dragRow.locator('.frame-color-track').boundingBox(),dragThumb=await dragRow.locator('.frame-color-thumb').boundingBox();
  const beforeDrag=await hex.inputValue();
  await page.mouse.move(dragThumb.x+dragThumb.width/2,dragThumb.y+dragThumb.height/2);await page.mouse.down();
  await page.mouse.move(dragTrack.x+dragTrack.width*.85,dragTrack.y+dragTrack.height/2,{steps:8});
  assert.notEqual(await hex.inputValue(),beforeDrag,'dragging updates the color before release');
  await page.mouse.up();
  await setHex('80402070');await number('Intensity',1);assert.equal(await hex.inputValue(),'b05a2f70');
  assert.equal(Number(await value('Intensity')),1);
  await mode('HSV');assert.equal(Number(await value('Intensity')),1);
  await number('Intensity',0);assert.equal(await hex.inputValue(),'80402070');
  await number('Hue',120);await number('Saturation',100);await number('Value',100);assert.equal(await hex.inputValue(),'00ff0070');
  await number('Alpha',64);assert.equal(await hex.inputValue(),'00ff0040');
  await slider('Hue',.66);assert(Number(await value('Hue'))>200,'hue='+await value('Hue')+',hex='+await hex.inputValue());
  await mode('RGB');assert.equal(await value('Alpha'),'64');
  await setHex('80808080');await mode('Linear');
  assert(Math.abs(Number(await value('Red'))-.216)<.002);
  await number('Red',.5);assert((await hex.inputValue()).startsWith('bc8080'));
  assert.equal(Number(await value('Intensity')),0);
  await number('Alpha',.25);assert((await hex.inputValue()).endsWith('40'));
  await slider('Green',.5);assert(Math.abs(Number(await value('Green'))-.5)<.02);
  await mode('RGB');assert(Math.abs(Number(await value('Green'))-188)<4);
  const primary=await page.getByRole('button',{name:/^primary color/}).getAttribute('style');
  await page.getByRole('button',{name:/^secondary color/}).click();await number('Red',55);
  assert.equal(await page.getByRole('button',{name:/^primary color/}).getAttribute('style'),primary);
  assert((await hex.inputValue()).startsWith('37'));
  await page.getByRole('button',{name:/^primary color/}).click();assert((await hex.inputValue()).startsWith('bc'));
  await number('Intensity',2);await number('Alpha',100);assert.equal(Number(await value('Intensity')),2);
  await number('Intensity',0);assert((await hex.inputValue()).startsWith('bc'));assert((await hex.inputValue()).endsWith('64'));
  const row=panel.locator('.color-channel-row').first();const rowBox=await row.boundingBox();const field=await row.locator('.frame-number-field').boundingBox();assert(field.x+field.width<=rowBox.x+rowBox.width+1);
  await page.setViewportSize({width:850,height:900});await mode('HSV');
  assert.equal(await panel.locator('.frame-number-field:visible').count(),5);
  fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/advanced-colors.png'});
  assert.deepEqual(errors,[]);
  console.log('PASS: live RGB/HSV/Linear sliders and Frame number fields, channel gradients, reversible linear-light intensity, alpha, target isolation, and narrow layout.');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exit(1)});
