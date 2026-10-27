import {expect,test} from 'bun:test';
import {brush} from '../../src/editor/tools/raster';
import {createToolOptions} from '../../src/editor/tools/toolOptions';
import {toolInfo,toolForKey} from '../../src/editor/tools/toolInfo';
import {tools} from '../../src/editor/tools/raster';
import {selectionBrushMask,pickedChannels} from '../../src/editor/tools/toolPixels';
import {drawToolText} from '../../src/editor/tools/textDrawing';
test('tool options are independent for each button and shape',()=>{
 const left=createToolOptions('left'),right=createToolOptions('right');left.bucket.tolerance=42;left.shapes.line.size=9;
 expect(right.bucket.tolerance).toBe(0);expect(left.shapes.curve.size).toBe(1);expect(right.zoom).toBe('out');
});
test('brush shape, hollow interior and overwrite modify actual pixels',()=>{
 const pixels=new Uint8ClampedArray(7*7*4);
 brush(pixels,7,7,[{x:3,y:3}],'#ff0000',{size:5,density:100,opacity:100,shape:'circle',filled:false});
 expect(pixels[(3*7+3)*4+3]).toBe(0);expect(pixels[(1*7+3)*4+3]).toBe(255);expect(pixels[(1*7+1)*4+3]).toBe(0);
 const color=new Uint8ClampedArray([0,0,255,255]);brush(color,1,1,[{x:0,y:0}],'#ff000080',{size:1,density:100,opacity:100,overwrite:true});expect([...color]).toEqual([255,0,0,128]);
});
test('brush spacing inserts real gaps between stamps',()=>{
 const pixels=new Uint8ClampedArray(10*4);
 brush(pixels,10,1,[{x:0,y:0},{x:9,y:0}],'#ffffff',{size:1,density:100,opacity:100,spacing:true,gapX:2,gapY:2});
 expect([...pixels].filter((_,i)=>i%4===3)).toEqual([255,0,0,255,0,0,255,0,0,255]);
});
test('Fill inside fills the interior of a pencil stroke rather than changing its stamp',()=>{
 const pixels=new Uint8ClampedArray(8*8*4);
 brush(pixels,8,8,[{x:1,y:1},{x:6,y:1},{x:6,y:6},{x:1,y:6},{x:1,y:1}],'#ffffff',{size:1,density:100,opacity:100,fillInside:true});
 expect(pixels[(3*8+3)*4+3]).toBe(255);expect(pixels[0]).toBe(0);
});
test('every tool has accurate tooltip metadata and unique assignment shortcuts',()=>{
 const keys=new Set<string>();for(const tool of tools){expect(toolInfo[tool.id].description.length).toBeGreaterThan(0);const key=toolInfo[tool.id].key;if(key){expect(keys.has(key)).toBe(false);keys.add(key);expect(toolForKey(key)).toBe(tool.id);}}
});
test('selection brush paints pixels without filling a freehand polygon',()=>{
 const mask=selectionBrushMask(10,10,[{x:1,y:1},{x:7,y:1},{x:7,y:7}],1,'square');
 expect(mask.has(1*10+4)).toBe(true);expect(mask.has(4*10+4)).toBe(false);
});
test('eyedropper channel modes preserve the requested channels',()=>{
 expect(pickedChannels('#ff000080','#12345640','rgb')).toBe('#ff000040');
 expect(pickedChannels('#ff000080','#123456','alpha')).toBe('#12345680');
});
test('text options configure real canvas font, alignment and multi-line placement',()=>{
 const calls:unknown[]=[];
 const ctx={font:'',textBaseline:'',fillStyle:'',textAlign:'',measureText:(value:string)=>({width:value.length*5}),fillText:(...args:unknown[])=>calls.push(args)};
 drawToolText(ctx as unknown as CanvasRenderingContext2D,{...createToolOptions('left').text,content:'A\nB',font:'serif',bold:true,italic:true,size:10,align:'right'},{x:12,y:2},'#123456');
 expect(ctx.font).toBe('italic bold 10px serif');expect(ctx.textAlign).toBe('right');expect(calls).toEqual([['A',12,2],['B',12,14]]);
});
