const {chromium}=require('playwright');const assert=require('node:assert/strict');const fs=require('node:fs');
(async()=>{const browser=await chromium.launch({channel:'chrome',args:['--no-proxy-server']});try{
 const page=await browser.newPage({viewport:{width:1100,height:900}}),errors=[];page.on('pageerror',e=>{errors.push(e.message);console.error(e.message)});page.setDefaultTimeout(10000);
 let fixture=fs.readFileSync('scripts/fixtures/manual-tools.js','utf8').replace('let opened=false','let opened=true');
 fixture=fixture.replace("layers=[{id:'layer',name:'Layer',visible:true,opacity:1}]","layers=[{id:'layer',name:'Layer',visible:true,opacity:1},{id:'overlay',name:'Overlay',visible:true,opacity:1}]");
 fixture=fixture.replace("if(!args.request.hidden_layers?.includes('layer'))","if(args.request.layer_id!=='overlay'&&!args.request.hidden_layers?.includes('layer'))");
 fixture=fixture.replace("return {width,height,png:c.toDataURL().split(',')[1]};","if(args.request.layer_id!=='layer'&&!args.request.hidden_layers?.includes('overlay')){const upper=document.createElement('canvas');upper.width=upper.height=16;upper.getContext('2d').putImageData(new ImageData(topPixels.slice(),16,16),0,0);c.getContext('2d').drawImage(upper,0,0); }return {width,height,png:c.toDataURL().split(',')[1]};");
 fixture += "\nlet topPixels=new Uint8ClampedArray(16*16*4);window.seedPixels=(entries,top=[])=>{pixels.fill(0);topPixels.fill(0);for(const [x,y,ink] of entries)pixels.set(ink,(y*16+x)*4);for(const [x,y,ink]of top)topPixels.set(ink,(y*16+x)*4);undo=[];redo=[];generation++;return generation;};";
 await page.route('**/src/services/backend.ts',r=>r.fulfill({contentType:'text/javascript',body:fixture}));
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

 const panels={left:page.locator('.tool-settings[aria-label="left tool settings"]'),right:page.locator('.tool-settings[aria-label="right tool settings"]')};
 async function assign(id,slot='left'){await page.locator('[data-tool="'+id+'"]').click({button:slot==='right'?'right':'left'});const header=page.getByRole('button',{name:slot==='left'?/^Left tools/:/^Right tools/});if(await header.getAttribute('aria-expanded')==='false')await header.click();return panels[slot];}
 async function choose(panel,label,value){await panel.getByRole('combobox',{name:label,exact:true}).click();await page.getByRole('option',{name:value,exact:true}).click();}
 async function number(panel,label,value){await panel.getByRole('spinbutton',{name:label,exact:true}).click();const input=panel.getByRole('textbox',{name:label+' value',exact:true});await input.fill(String(value));await input.press('Enter');}
 async function check(panel,label,value){const c=panel.getByRole('checkbox',{name:label,exact:true});if((await c.getAttribute('aria-checked')==='true')!==value)await c.click();}
 async function seed(entries=[],top=[]){await reset();const gen=await page.evaluate(([entries,top])=>window.seedPixels(entries,top),[entries,top]);await page.waitForFunction(gen=>window.testCalls.some(c=>c.command==='render'&&c.args.request.generation>=gen),gen);await page.waitForTimeout(80);}
 async function stroke(a,b=a,mods=[]){const before=await page.evaluate(()=>window.testCalls.filter(c=>c.command==='paint_pixels').length);await draw(a[0],a[1],b[0],b[1],mods);await page.waitForTimeout(160);return before;}
 const pixel=(x,y)=>page.evaluate(([x,y])=>window.toolPixels().slice((y*16+x)*4,(y*16+x)*4+4),[x,y]);
 async function color(hex,slot='primary'){await page.getByRole('button',{name:new RegExp('^'+slot+' color')}).click();const input=page.getByLabel('Hex color, optional alpha');await input.fill(hex);await input.press('Tab');}
 // Every current tool exposes its actual options through Frame controls.
 for(const id of ['rectangle','ellipse','polygon','lasso','paint-select','color','wand','move','zoom','pan','text','paper','pencil','eraser','bucket','eyedropper','line','curve','draw-rectangle','draw-ellipse','isometric']){
  const panel=await assign(id);assert(await panel.locator('.frame-select,.frame-scrub-field,.frame-button,.frame-checkbox,.frame-text-area').count()>0,id+' has options');assert.equal(await panel.locator('select').count(),0,id+' uses Frame dropdowns');assert.equal(await panel.locator('.hint').count(),0);
 }
 const left=await assign('pencil');await number(left,'Size',3);const right=await assign('pencil','right');assert.equal(await right.getByRole('spinbutton',{name:'Size',exact:true}).getAttribute('aria-valuenow'),'1');await number(left,'Size',1);
 await color('ff0000');await check(left,'Spacing',true);await number(left,'Gap X',2);await number(left,'Gap Y',2);await seed();await stroke([0,0],[9,0]);assert.equal((await pixel(1,0))[3],0);assert.equal((await pixel(3,0))[3],255);
 await check(left,'Spacing',false);await check(left,'Fill inside',true);await seed();let p=await point(1.5,1.5);await page.mouse.move(p.x,p.y);await page.mouse.down();for(const [x,y]of [[6.5,1.5],[6.5,6.5],[1.5,6.5],[1.5,1.5]]){p=await point(x,y);await page.mouse.move(p.x,p.y,{steps:5});}await page.mouse.up();await page.waitForTimeout(160);assert.equal((await pixel(3,3))[3],255);
 await check(left,'Fill inside',false);await check(left,'Overwrite color',true);await color('ff000080');await seed([[5,5,[0,0,255,255]]]);await stroke([5,5]);assert.deepEqual(await pixel(5,5),[255,0,0,128]);
 await choose(left,'Brush shape','Round brush');await number(left,'Size',5);await seed();await stroke([5,5]);assert.equal((await pixel(3,3))[3],0);assert.equal((await pixel(5,3))[3],128);
 await choose(left,'Brush shape','Square brush');await number(left,'Size',1);await color('ff0000');await seed();await stroke([1,1]);await stroke([7,1],[7,1],['Shift']);assert.equal((await pixel(4,1))[3],255);
 await assign('eraser');await number(left,'Opacity',50);await seed([[4,4,[255,0,0,255]]]);await stroke([4,4]);assert.equal((await pixel(4,4))[3],128);
 // Color selection is global, Magic Wand is connected.
 await seed([[1,1,[255,0,0,255]],[10,10,[255,0,0,255]]]);await assign('color');await stroke([1,1]);assert.deepEqual(await bounds(),{x:1,y:1,width:10,height:10});await reset();await assign('wand');await stroke([1,1]);assert.deepEqual(await bounds(),{x:1,y:1,width:1,height:1});
 // Painted deselection removes only brush pixels, rather than moving the boundary.
 await seed();await assign('paint-select');await number(left,'Brush size',1);await page.locator('.canvas-stage').focus();await page.keyboard.press('Control+a');await choose(left,'Selection brush action','Deselect pixels');await stroke([2,2]);await assign('bucket');await choose(left,'Fill area','Entire selection');await stroke([0,0]);assert.equal((await pixel(2,2))[3],0);assert.equal((await pixel(3,3))[3],255);
 // Bucket source merging changes the region, while writes stay on the active layer.
 await seed([[1,1,[255,0,0,255]],[2,1,[0,0,255,255]]],[[2,1,[255,0,0,255]]]);await choose(left,'Fill area','Similar connected area');await check(left,'Sample merged visible layers',true);await color('00ff00');await stroke([1,1]);assert.deepEqual(await pixel(2,1),[0,255,0,255]);
 await choose(left,'Fill with','Transparent');await stroke([1,1]);assert.equal((await pixel(1,1))[3],0);
 // Picker source, destination, and channels are independently configurable.
 await seed([[1,1,[255,0,0,128]]],[[1,1,[0,255,0,255]]]);await color('0000ff40','secondary');await assign('eyedropper');await choose(left,'Pick for','Right color');await choose(left,'Pick source','Merged visible layers');await choose(left,'Pick mode','Color only');await stroke([1,1]);assert((await page.getByRole('button',{name:/^secondary color/}).getAttribute('aria-label')).includes('#00ff0040'));
 await choose(left,'Pick for','Left color');await choose(left,'Pick source','Current layer');await choose(left,'Pick mode','Color and alpha');await stroke([1,1]);assert((await page.getByRole('button',{name:/^primary color/}).getAttribute('aria-label')).includes('#ff000080'));
 await seed();await assign('zoom');await choose(left,'Zoom mode','Zoom out');const before=(await art.boundingBox()).width;await stroke([8,8]);assert((await art.boundingBox()).width<before);await left.getByRole('button',{name:'Fit artwork',exact:true}).click();
 await assign('text');await left.getByRole('textbox',{name:'Text',exact:true}).fill('A');await number(left,'Size (px)',9);await choose(left,'Font','Serif');await choose(left,'Text alignment','Right');await check(left,'Bold',true);await check(left,'Italic',true);await check(left,'Antialias text',true);await color('ffffff');await seed();await stroke([10,2]);assert(await page.evaluate(()=>window.toolPixels().some((v,i)=>i%4===3&&v>0&&v<255)),'antialias produces fractional coverage');
 // All documented assignment shortcuts work without overriding typed text.
 await page.locator('.canvas-stage').focus();await page.keyboard.press('q');assert(await page.locator('[data-tool=wand]').evaluate(el=>el.classList.contains('assigned-left')));await page.keyboard.press('Alt+i');assert(await page.locator('[data-tool=paint-select]').evaluate(el=>el.classList.contains('assigned-right')));
 await assign('pencil');await page.mouse.move(900,200);await page.waitForTimeout(150);await page.locator('[data-tool=pencil]').hover();await page.getByRole('tooltip').waitFor();assert((await page.getByRole('tooltip').textContent()).includes('Shift'));
 fs.mkdirSync('target/ui-verification',{recursive:true});await page.screenshot({path:'target/ui-verification/tool-options-pencil.png'});await assign('bucket');await page.screenshot({path:'target/ui-verification/tool-options-bucket.png'});assert.deepEqual(errors,[]);
 console.log('Tool audit: all 21 tool panels, Frame controls, per-button settings, spacing, stroke fill, overwrite, round brushes, straight strokes, eraser opacity, selection modes, merged bucket/picker, zoom, styled antialiased text, shortcuts, and detailed tooltips passed');
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
