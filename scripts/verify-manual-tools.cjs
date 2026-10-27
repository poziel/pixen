// Run against a Vite server. Storage is mocked; gestures and pixel operations are real.
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});
 try{
  const page=await browser.newPage({viewport:{width:1280,height:900}}),errors=[];
  page.setDefaultTimeout(10000);page.on('pageerror',e=>{errors.push(e.message);console.error(e.message)});page.on('console',m=>{if(m.type()==='error')console.error(m.text())});
  await page.route('**/src/services/backend.ts',route=>route.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/manual-tools.js','utf8')}));
  await page.route('**/@tauri-apps_api_window.js*',route=>route.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
  await page.route('**/@tauri-apps_api_event.js*',route=>route.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
  await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1459');
  await page.locator('.pane-welcome-actions').getByRole('button',{name:/Open artwork/}).click();
  if (!await page.locator('.color-picker').isVisible()) await page.getByRole('button',{name:'Toggle primary sidebar',exact:true}).click();await page.getByRole('button',{name:'Tools',exact:true}).click();
  await page.locator('.color-picker').waitFor();
  const toolbar=page.getByRole('toolbar',{name:'Drawing tools'});
  await toolbar.waitFor();
  assert.equal(await page.locator('.pixel-tools .tool-grid').count(),0);
  const buttons=await toolbar.locator('button').all();
  const boxes=await Promise.all(buttons.map(button=>button.boundingBox()));
  assert(boxes.length>=13);
  assert(boxes.every((box,i)=>box && Math.abs(box.y-boxes[0].y)<1 && (!i||box.x>boxes[i-1].x)));
  const dock=await page.locator('[data-dock-id="pixen.drawing-tools"]').boundingBox();
  const canvas=await page.locator('.canvas-stage').boundingBox();
  const workspace=await page.locator('.workspace-region').boundingBox();
  assert(dock.y>=workspace.y&&dock.y+dock.height<=workspace.y+workspace.height+1);
  assert.equal(await page.locator('[data-dock-host="pixen.drawing-tools"]').evaluate(el=>el.closest('[data-dock-placement]').dataset.dockPlacement),'overlay');


  async function assign(slot,id){
    await toolbar.locator('[data-tool="'+id+'"]').click({button:slot==='right'?'right':'left'});
    const section=page.getByRole('button',{name:slot==='left'?/^Left tools/:/^Right tools/});
    if(await section.getAttribute('aria-expanded')==='false')await section.click();
  }
  async function selectPicker(mode){await page.getByRole('button',{name:'Color picker type',exact:true}).click();await page.getByRole('menuitemcheckbox',{name:{rectangle:'HSV Rectangle',wheel:'HSV Wheel',circle:'VHS Circle'}[mode],exact:true}).click();}
  const art=page.locator('canvas.artwork:not(.selection-overlay)');
  await page.waitForFunction(()=>document.querySelector('canvas.artwork')?.width===16);
  const pos=async(x,y)=>{const b=await art.boundingBox();return {x:b.x+(x+.5)*b.width/16,y:b.y+(y+.5)*b.height/16};};
  async function stroke(a,b=a,button='left'){
    const p=await pos(...a),q=await pos(...b),before=await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length);
    await page.mouse.move(p.x,p.y);await page.mouse.down({button});await page.mouse.move(q.x,q.y,{steps:8});await page.mouse.up({button});
    await page.waitForTimeout(180);return before;
  }
  const pixel=(x,y)=>page.evaluate(([x,y])=>window.toolPixels().slice((y*16+x)*4,(y*16+x)*4+4),[x,y]);
  await stroke([1,1],[5,1]);assert.equal((await pixel(3,1))[3],255);
  await stroke([3,1],undefined,'right');assert.equal((await pixel(3,1))[3],0);
  await page.getByRole('button',{name:'Back',exact:true}).click();await page.waitForTimeout(120);assert.equal((await pixel(3,1))[3],255);
  await page.getByRole('button',{name:'Forward',exact:true}).click();await page.waitForTimeout(120);assert.equal((await pixel(3,1))[3],0);
  await assign('left','rectangle');await stroke([8,8],[10,10]);
  await assign('left','paper');await stroke([9,9]);assert.equal((await pixel(9,9))[3],255);assert.equal((await pixel(7,9))[3],0);
  await assign('left','move');await stroke([9,9],[11,9]);assert.equal((await pixel(8,9))[3],0);assert.equal((await pixel(12,9))[3],255);
  await page.locator('.canvas-stage').focus();await page.keyboard.press('Escape');
  const before=await page.getByLabel('Hex color, optional alpha').inputValue();await page.locator('.canvas-stage').focus();await page.keyboard.press('x');assert.notEqual(await page.getByLabel('Hex color, optional alpha').inputValue(),before);

  for(const mode of ['wheel','circle','rectangle']){await selectPicker(mode);const area=page.locator('.frame-hsv-area');const b=await area.boundingBox();await area.click({position:{x:b.width*.7,y:b.height*.4}});}
  const count=await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length),transform=await art.evaluate(c=>c.style.transform);
  await page.locator('.canvas-stage').focus();await page.keyboard.down('Enter');await stroke([6,6],[8,7]);await page.keyboard.up('Enter');
  assert.equal(await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length),count);assert.notEqual(await art.evaluate(c=>c.style.transform),transform);
  await assign('left','text');await page.getByRole('textbox',{name:'Text',exact:true}).fill('A');const textOptions=page.locator('.tool-settings').filter({has:page.getByRole('textbox',{name:'Text',exact:true})});await textOptions.getByRole('spinbutton',{name:'Size (px)',exact:true}).click();await textOptions.getByRole('textbox',{name:'Size (px) value',exact:true}).fill('8');await textOptions.getByRole('textbox',{name:'Size (px) value',exact:true}).press('Enter');await stroke([1,4]);
  assert(await page.evaluate(()=>window.testCalls.some(c=>c.command==='paint_pixels'&&c.args.pixels.length>9)));
  // Low-alpha text must survive the pixel coverage threshold.
  await page.getByLabel('Hex color, optional alpha').fill('#ff000040');await page.getByLabel('Hex color, optional alpha').press('Tab');
  await stroke([7,1]);assert(await page.evaluate(()=>window.toolPixels().some((v,i)=>i%4===3&&v===64)));
  async function clearArtwork(){
    await page.locator('.canvas-stage').focus();await page.keyboard.press('Escape');
    await assign('left','paper');
    await page.getByLabel('Hex color, optional alpha').fill('#00000000');await page.getByLabel('Hex color, optional alpha').press('Tab');await stroke([1,1]);
    await page.getByLabel('Hex color, optional alpha').fill('#ff0000');await page.getByLabel('Hex color, optional alpha').press('Tab');
  }
  await clearArtwork();
  await assign('left','ellipse');await stroke([4,4],[8,8]);
  await page.waitForTimeout(400); // Selection must survive the background snapshot poll.
  await assign('left','paper');await stroke([6,6]);assert.equal((await pixel(6,6))[3],255);assert.equal((await pixel(4,4))[3],0);
  await clearArtwork();
  await assign('right','polygon');
  for(const p of [[2,2],[6,2],[2,6]])await stroke(p,undefined,'right');
  const vertex=await pos(2,6);await page.mouse.click(vertex.x,vertex.y,{button:'right'});await page.waitForTimeout(100);
  await stroke([3,3]);assert.equal((await pixel(3,3))[3],255);assert.equal((await pixel(6,6))[3],0);
  await page.locator('.canvas-stage').focus();await page.keyboard.press('Escape');
  await assign('left','color');await stroke([3,3]);
  await page.getByLabel('Hex color, optional alpha').fill('#0000ff');await page.getByLabel('Hex color, optional alpha').press('Tab');
  await assign('left','paper');await stroke([3,3]);assert.deepEqual(await pixel(3,3),[0,0,255,255]);
  await page.locator('.canvas-stage').focus();await page.keyboard.press('Escape');
  await assign('left','bucket');await stroke([12,12]);assert.deepEqual(await pixel(12,12),[0,0,255,255]);
  await assign('left','eyedropper');await stroke([3,3]);assert.equal(await page.getByLabel('Hex color, optional alpha').inputValue(),'0000ffff');
  await assign('left','zoom');const oldTransform=await art.evaluate(c=>c.style.transform);await stroke([6,6]);assert.notEqual(await art.evaluate(c=>c.style.transform),oldTransform);
  await page.getByRole('button',{name:'Fit artwork',exact:true}).click();
  // Click tools retain their result when the pointer moves after loading.
  await clearArtwork();
  const start=await pos(2,2),end=await pos(3,3);
  await page.mouse.move(start.x,start.y);await page.mouse.down();await page.waitForTimeout(180);await page.mouse.move(end.x,end.y);await page.mouse.up();await page.waitForTimeout(180);
  assert.equal((await pixel(2,2))[3],255);
  // Hue remains adjustable even when saturation/value collapse to black.
  await page.getByLabel('Hex color, optional alpha').fill('#000000');await page.getByLabel('Hex color, optional alpha').press('Tab');
  await page.getByRole('tab',{name:'HSV',exact:true}).click();
  await page.getByLabel('Hue value',{exact:true}).fill('120');await page.getByLabel('Hue value',{exact:true}).press('Tab');
  assert.equal(await page.getByLabel('Hue value',{exact:true}).inputValue(),'120');
  await assign('left','pencil');
  // Assign the same tool to both buttons, with independent sizes and colors.
  await page.getByRole('button',{name:/^Pencil/}).click({button:'right'});
  const pencil=page.getByRole('button',{name:/^Pencil/});
  assert(await pencil.evaluate(el=>el.classList.contains('assigned-left')&&el.classList.contains('assigned-right')));
  const leftSize=page.getByRole('region',{name:'left tool settings'}).getByRole('spinbutton',{name:'Size',exact:true});
  const rightSize=page.getByRole('region',{name:'right tool settings'}).getByRole('spinbutton',{name:'Size',exact:true});
  async function brushSize(slot,value){const scope=page.getByRole('region',{name:slot+' tool settings'});await scope.getByRole('spinbutton',{name:'Size',exact:true}).click();const input=scope.getByRole('textbox',{name:'Size value',exact:true});await input.fill(value);await input.press('Enter');}
  await brushSize('left','1');await brushSize('right','3');
  const cursorPoint=await pos(6,6);await page.mouse.move(cursorPoint.x,cursorPoint.y);
  await page.waitForFunction(()=>document.querySelector('.tool-cursor [data-slot=left]')?.getAttribute('data-pixel-x')==='6');
  assert.equal(await page.locator('.tool-cursor [data-slot=left]').getAttribute('data-size'),'1');
  assert.equal(await page.locator('.tool-cursor [data-slot=right]').getAttribute('data-size'),'3');
  const zoomBeforeResize=await art.evaluate(c=>c.style.transform);
  await page.keyboard.down('Control');await page.mouse.wheel(0,-100);await page.keyboard.up('Control');
  assert.equal(await leftSize.getAttribute('aria-valuenow'),'2');assert.equal(await rightSize.getAttribute('aria-valuenow'),'3');
  await page.keyboard.down('Control');await page.keyboard.down('Shift');await page.mouse.wheel(0,-100);await page.keyboard.up('Shift');await page.keyboard.up('Control');
  assert.equal(await rightSize.getAttribute('aria-valuenow'),'4');assert.equal(await leftSize.getAttribute('aria-valuenow'),'2');
  assert.equal(await art.evaluate(c=>c.style.transform),zoomBeforeResize);
  await page.locator('.canvas-stage').focus();
  const paintsBeforePan=await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length);
  const beforeSpacePan=await art.evaluate(c=>c.style.transform);
  await page.keyboard.down('Space');await page.keyboard.down('Space');
  await page.waitForFunction(()=>document.querySelector('.tool-grid button[data-tool=pan]')?.classList.contains('assigned-left'));
  assert.equal(await page.locator('.tool-cursor g').count(),0);
  assert.equal(await page.getByRole('button',{name:/Pause.*playback/,includeHidden:true}).count(),0);
  await page.mouse.move(cursorPoint.x,cursorPoint.y);await page.mouse.down();await page.mouse.move(cursorPoint.x+30,cursorPoint.y+20);await page.mouse.up();await page.keyboard.up('Space');
  await page.waitForFunction(()=>document.querySelector('.tool-grid button[data-tool=pencil]')?.classList.contains('assigned-left'));
  assert.notEqual(await art.evaluate(c=>c.style.transform),beforeSpacePan);
  assert.equal(await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length),paintsBeforePan);
  assert(await pencil.evaluate(el=>el.classList.contains('assigned-left')));
  // A later ordinary gesture draws rather than getting stuck in temporary pan.
  await brushSize('left','1');await brushSize('right','1');
  await page.getByLabel('Hex color, optional alpha').fill('#ff0000');await page.getByLabel('Hex color, optional alpha').press('Tab');
  await page.getByRole('button',{name:/^secondary color/}).click();await page.getByLabel('Hex color, optional alpha').fill('#0000ff');await page.getByLabel('Hex color, optional alpha').press('Tab');
  await stroke([4,4]);await stroke([5,5],undefined,'right');
  assert.deepEqual(await pixel(4,4),[255,0,0,255]);assert.deepEqual(await pixel(5,5),[0,0,255,255]);
  await brushSize('left','2');await brushSize('right','4');
  await page.locator('.color-picker').scrollIntoViewIfNeeded();
  const finalPoint=await pos(7,7);await page.mouse.move(finalPoint.x,finalPoint.y);
  const screenScale=(await art.boundingBox()).width/16;
  const outlineStroke=Number(await page.locator('.tool-cursor [data-slot=left] path').last().getAttribute('stroke-width'));
  assert(Math.abs(outlineStroke*screenScale-1.5)<.01);
  await page.getByRole('button',{name:/^primary color/}).click();
  await page.getByRole('button',{name:'Pick color from artwork',exact:true}).click();
  await stroke([5,5]);
  assert.equal(await page.getByLabel('Hex color, optional alpha').inputValue(),'0000ffff');
  assert(await pencil.evaluate(el=>el.classList.contains('assigned-left')&&el.classList.contains('assigned-right')));
  assert.equal(await page.getByRole('button',{name:'Pick color from artwork',exact:true}).getAttribute('aria-pressed'),'false');
  assert(await page.getByRole('group',{name:'Recent colors'}).locator('button').count()>0);
  await page.getByRole('tab',{name:'RGB',exact:true}).click();
  await page.getByLabel('Red value',{exact:true}).fill('128');await page.getByLabel('Red value',{exact:true}).press('Tab');
  assert((await page.getByLabel('Hex color, optional alpha').inputValue()).startsWith('80'));
  assert.equal(await page.getByLabel('Left button',{exact:true}).count(),0);
  assert.deepEqual(errors,[]);
  fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/manual-tools.png'});
  console.log('PASS: drawing regression plus Space/Enter pan, no Space playback toggle, Ctrl+wheel independent sizes without zoom, thin pixel-aligned dual cursors, split toolbar assignments, and primary/secondary pencil colors.');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exit(1)});
