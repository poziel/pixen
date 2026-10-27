const {chromium}=require('playwright');const assert=require('node:assert/strict');const fs=require('node:fs');
(async()=>{const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});try{
 const page=await browser.newPage({viewport:{width:1100,height:900}}),errors=[];page.on('pageerror',e=>{errors.push(e.message);console.error(e.message)});page.setDefaultTimeout(10000);
 await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace('let opened=false','let opened=true')}));
 await page.route('**/@tauri-apps_api_window.js*',r=>r.fulfill({contentType:'text/javascript',body:'export function getCurrentWindow(){return {onDragDropEvent:async()=>()=>{},onCloseRequested:async()=>()=>{},close:async()=>{}}}'}));
 await page.route('**/@tauri-apps_api_event.js*',r=>r.fulfill({contentType:'text/javascript',body:'export async function listen(){return ()=>{}}'}));
 await page.goto(process.env.PIXEN_TEST_URL||'http://127.0.0.1:1463');
 const art=page.locator('canvas.artwork:not(.selection-overlay)');await art.waitFor();await page.waitForFunction(()=>document.querySelector('canvas.artwork')?.width===16);
 const overlay=page.locator('.selection-boundary');
 const bounds=()=>overlay.evaluate(el=>Object.fromEntries(['x','y','width','height'].map(k=>[k,el.hasAttribute(`data-selection-${k}`)?Number(el.getAttribute(`data-selection-${k}`)):null])));
 async function point(x,y){const b=await art.boundingBox();return {x:b.x+x*b.width/16,y:b.y+y*b.height/16};}
 async function drag(a,b,mods=[]){for(const m of mods)await page.keyboard.down(m);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:6});await page.mouse.up();for(const m of [...mods].reverse())await page.keyboard.up(m);}
 async function draw(x,y,r,b,mods=[]){await drag(await point(x+.5,y+.5),await point(r+.5,b+.5),mods);}
 async function tool(id){await page.locator(`[data-tool="${id}"]`).click();}
 async function reset(){await page.locator('.canvas-stage').focus();await page.keyboard.press('Control+d');}
 async function handle(id,dx,dy,mods=[]){const box=await page.locator(`[data-selection-handle="${id}"]`).boundingBox(),a={x:box.x+box.width/2,y:box.y+box.height/2},b=await art.boundingBox();for(const m of mods)await page.keyboard.down(m);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(a.x+dx*b.width/16,a.y+dy*b.height/16,{steps:6});await page.mouse.up();for(const m of [...mods].reverse())await page.keyboard.up(m);}

 await tool('rectangle');
 await page.getByRole('button',{name:/^Left tools · Rectangular Selection$/}).click();
 const emptyFields=page.locator('.tool-parameters').first();
 assert(await emptyFields.getByRole('group',{name:'Position',exact:true}).isVisible());
 assert(await emptyFields.getByRole('group',{name:'Size',exact:true}).isVisible());
 for(const name of ['X','Y','Width','Height']){const field=emptyFields.getByRole('spinbutton',{name,exact:true});assert(await field.isDisabled());assert.equal(await field.getAttribute('aria-valuenow'),'0');}
 assert(await emptyFields.getByRole('button',{name:'Lock aspect ratio',exact:true}).isDisabled());
 await page.getByRole('button',{name:/^Left tools · Rectangular Selection$/}).click();
 // Both assignments on one tool retain separate, correctly positioned markers.
 await page.locator('[data-tool="rectangle"]').click({button:'right'});
 const markerSides=await page.locator('[data-tool="rectangle"]').evaluate(button=>{const b=button.getBoundingClientRect(),l=button.querySelector('.tool-assignment-left').getBoundingClientRect(),r=button.querySelector('.tool-assignment-right').getBoundingClientRect();return l.x+l.width/2<b.x+b.width/2&&r.x+r.width/2>b.x+b.width/2;});assert(markerSides);
 await page.locator('[data-tool="eraser"]').click({button:'right'});
 await draw(2,3,5,6);assert.deepEqual(await bounds(),{x:2,y:3,width:4,height:4});assert.equal(await page.locator('[data-selection-handle]').count(),8);assert(await page.locator('[data-selection-handle]').evaluateAll(nodes=>nodes.every(el=>{const b=el.getBoundingClientRect();return b.width<=8&&b.height<=8})), 'handles remain screen-sized at high zoom');
 const ants=page.locator('.selection-ants'),offset=await ants.evaluate(el=>getComputedStyle(el).strokeDashoffset);await page.waitForTimeout(120);assert.notEqual(await ants.evaluate(el=>getComputedStyle(el).strokeDashoffset),offset);
 await handle('e',2,0);assert.deepEqual(await bounds(),{x:2,y:3,width:6,height:4});
 await reset();await draw(3,3,6,6);await handle('e',2,0,['Shift']);assert.deepEqual(await bounds(),{x:3,y:2,width:6,height:6});
 await reset();await draw(3,3,6,6);await handle('nw',-1,-1);assert.deepEqual(await bounds(),{x:2,y:2,width:5,height:5});
 await reset();await draw(3,3,6,6);await handle('e',1,0,['Control']);assert.deepEqual(await bounds(),{x:2,y:3,width:6,height:4});
 await reset();await draw(3,3,6,6);await handle('nw',-1,-2,['Control']);assert.deepEqual(await bounds(),{x:2,y:1,width:6,height:8});
 await reset();await draw(3,3,6,6);await handle('e',1,0,['Control','Shift']);assert.deepEqual(await bounds(),{x:2,y:2,width:6,height:6});
 await drag(await point(4.5,4.5),await point(6.5,5.5));assert.deepEqual(await bounds(),{x:4,y:3,width:6,height:6});
 // Modifier drawing adds disconnected regions and subtracts from existing ones.
 await reset();await draw(1,1,3,3);await draw(8,8,9,9,['Shift']);assert.deepEqual(await bounds(),{x:1,y:1,width:9,height:9});
 await draw(8,8,9,9,['Control','Shift']);assert.deepEqual(await bounds(),{x:1,y:1,width:3,height:3});
 // Control held before drawing intersects, even when starting inside the current mask.
 await reset();await draw(1,1,5,5);await draw(3,3,7,7,['Control']);assert.deepEqual(await bounds(),{x:3,y:3,width:3,height:3});
 // Modifiers pressed after pointer-down affect placement geometry, not combination mode.
 for(const kind of ['rectangle','ellipse'])for(const mods of [[],['Shift'],['Control'],['Control','Shift']]){
  await reset();await tool(kind);const a=await point(8.5,8.5),b=await point(10.5,9.5);
  await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:4});
  for(const m of mods)await page.keyboard.down(m);
  const centered=mods.includes('Control'),square=mods.includes('Shift');
  assert.deepEqual(await bounds(),{x:centered?6:8,y:centered?(square?6:7):8,width:centered?5:3,height:square?(centered?5:3):(centered?3:2)});
  await page.mouse.up();for(const m of [...mods].reverse())await page.keyboard.up(m);
 }
 // The starting operation remains add after releasing/re-pressing Shift for geometry.
 await reset();await tool('rectangle');await draw(1,1,2,2);
 await page.keyboard.down('Shift');const a=await point(8.5,8.5),b=await point(10.5,9.5);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y);
 assert.deepEqual(await bounds(),{x:1,y:1,width:10,height:9});
 await page.keyboard.up('Shift');await page.keyboard.down('Shift');assert.deepEqual(await bounds(),{x:1,y:1,width:10,height:10});await page.mouse.up();await page.keyboard.up('Shift');
 // Shared geometry and modifiers apply to ellipse and polygon tools too.
 await reset();await tool('ellipse');await draw(3,3,6,6);await handle('s',0,2,['Shift']);assert.deepEqual(await bounds(),{x:2,y:3,width:6,height:6});
 await reset();await tool('polygon');
 for(const [x,y]of [[2,2],[7,2],[7,7]]){const p=await point(x+.5,y+.5);await page.mouse.click(p.x,p.y);}
 const last=await point(2.5,7.5);await page.mouse.dblclick(last.x,last.y);assert((await bounds()).width>0);
 await handle('e',1,0);assert.equal(await page.locator('[data-selection-handle]').count(),8);
 // Polygon intersection is latched at its first vertex, even after Control is released.
 await reset();await tool('rectangle');await draw(1,1,5,5);await tool('polygon');await page.keyboard.down('Control');
 let vertex=await point(3.5,3.5);await page.mouse.click(vertex.x,vertex.y);await page.keyboard.up('Control');
 for(const [x,y]of [[7.5,3.5],[7.5,7.5]]){vertex=await point(x,y);await page.mouse.click(vertex.x,vertex.y);}
 vertex=await point(3.5,7.5);await page.mouse.dblclick(vertex.x,vertex.y);assert.deepEqual(await bounds(),{x:3,y:3,width:3,height:3});
 await page.locator('.canvas-stage').focus();await page.keyboard.press('Control+a');assert.deepEqual(await bounds(),{x:0,y:0,width:16,height:16});
 // Numeric sidebar fields resize the boundary without writing pixels.
 await page.getByRole('button',{name:/^Left tools · Polygonal Selection$/}).click();
 const left=page.locator('.tool-parameters').first();const widthField=left.getByRole('spinbutton',{name:'Width',exact:true});await widthField.click();const widthInput=left.getByRole('textbox',{name:'Width value',exact:true});await widthInput.fill('12');await widthInput.press('Enter');assert.equal((await bounds()).width,12);

 assert.equal(await left.locator('select').count(),0,'selection mode uses FrameSelect');
 assert.equal(await left.getByRole('button',{name:/Deselect/}).count(),0);
 assert.equal(await left.locator('p').count(),0,'selection settings contain no help paragraphs');
 assert.equal(await left.getByRole('group',{name:'Position',exact:true}).count(),1);
 assert.equal(await left.getByRole('group',{name:'Size',exact:true}).count(),1);
 await left.getByRole('combobox',{name:'Selection mode'}).click();await page.getByRole('option',{name:'Add to selection',exact:true}).click();
 await left.getByRole('combobox',{name:'Selection mode'}).click();await page.getByRole('option',{name:'Replace selection',exact:true}).click();
 async function setField(name,value){await left.getByRole('spinbutton',{name,exact:true}).click();const input=left.getByRole('textbox',{name:name+' value',exact:true});await input.fill(String(value));await input.press('Enter');}
 await setField('Height',8);
 const lock=left.getByRole('button',{name:'Lock aspect ratio',exact:true});await lock.click();assert.equal(await lock.getAttribute('aria-pressed'),'true');
 await setField('Width',6);assert.deepEqual(await bounds(),{x:0,y:0,width:6,height:4});
 await setField('Height',6);assert.deepEqual(await bounds(),{x:0,y:0,width:9,height:6});
 await lock.click();await setField('Height',7);assert.equal((await bounds()).width,9);
 await setField('X',2);assert.deepEqual(await bounds(),{x:2,y:0,width:9,height:7});
 // Restore the original numeric regression baseline.
 await setField('X',0);await setField('Width',12);
 const hex=page.getByLabel('Hex color, optional alpha');await hex.focus();await page.keyboard.press('Control+a');assert.equal((await bounds()).width,12,'text field keeps Ctrl+A');
 assert.equal(await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length),0,'boundary transforms never edit artwork');
 await reset();assert.equal((await bounds()).width,null);
 // Pixel Move still moves artwork and the selected mask together.
 await tool('pencil');const pixel=await point(4.5,4.5);await page.mouse.click(pixel.x,pixel.y);await page.waitForFunction(()=>window.toolPixels()[(4*16+4)*4+3]>0);
 await tool('rectangle');await draw(2,2,6,6);await tool('move');
 const from=await point(4.5,4.5),to=await point(5.5,5.5);await page.mouse.move(from.x,from.y);await page.mouse.down();await page.waitForTimeout(100);await page.mouse.move(to.x,to.y,{steps:5});assert(await page.locator('.selection-guides').count());
 fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/selection-guides.png'});await page.mouse.up();
 await page.waitForFunction(()=>window.toolPixels()[(5*16+5)*4+3]>0);assert.equal(await page.evaluate(()=>window.toolPixels()[(4*16+4)*4+3]),0);assert.deepEqual(await bounds(),{x:3,y:3,width:5,height:5});
 await tool('rectangle');await page.screenshot({path:'target/ui-verification/selection-handles.png'});
 assert.deepEqual(errors,[]);console.log('Selections: animated borders, handles, anchored/proportional/centered resizing, boundary dragging, add/subtract/intersect, modifier ordering, live placement geometry, ellipse/polygon, numeric bounds, Ctrl+A/D, text focus, unchanged artwork, and pixel Move passed');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
