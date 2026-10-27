import {expect,test} from 'bun:test';
import {captureCels,planCelPaste} from '../../src/editor/timeline/celClipboard';
import {reactive,ref} from 'vue';
import {useCelClipboard} from '../../src/editor/timeline/useCelClipboard';
import type {Editor} from '../../src/editor/editor';
test('copied cels snapshot bytes and one cel broadcasts across unrelated destinations',async()=>{
 const pixels=[255,0,0,128];
 const copied=await captureCels([{layerId:'a',frameId:'one'}],['a','b'],['one','two'],1,1,async()=>({pixels,cel_opacity:.5}));
 pixels[0]=0;
 const ops=planCelPaste(copied,[{layerId:'b',frameId:'two'},{layerId:'b',frameId:'one'}],{layerId:'b',frameId:'two'},['a','b'],['one','two'],1,1);
 expect(ops).toHaveLength(1);expect(ops[0]!.pixels).toEqual([[255,0,0,128]]);expect(ops[0]!.frame_ids).toEqual(['two','one']);expect(ops[0]!.opacity).toBe(.5);
});
test('paste requested immediately after copy waits for the snapshot and uses the new destination',async()=>{
 let release!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve;});const operations:unknown[]=[];
 const editor={state:reactive({snapshot:{active_document:'art',managed:false,document:{generation:1,width:1,height:1,alternate_view:false,layers:[{id:'a'},{id:'b'}],frames:[{id:'one'},{id:'two'}]}},working:false,layer:'a',frame:0}),readCel:async()=>{await gate;return{pixels:[255,0,0,255],cel_opacity:1};},refresh:async()=>{}} as unknown as Editor;
 const selected=ref([{layerId:'a',frameId:'one'}]);
 const clipboard=useCelClipboard(editor,selected,async operation=>{operations.push(operation);},()=>{});
 const copying=clipboard.copy();selected.value=[{layerId:'b',frameId:'two'}];editor.state.layer='b';editor.state.frame=1;
 const pasting=clipboard.paste();expect(operations).toHaveLength(0);release();await copying;await pasting;
 expect((operations[0] as {layer_id:string;frame_ids:string[]}[])[0]).toMatchObject({layer_id:'b',frame_ids:['two']});
});
test('multi-cel paste retains sparse offsets and rejects overflow atomically',async()=>{
 const copied=await captureCels([{layerId:'a',frameId:'one'},{layerId:'b',frameId:'two'}],['a','b'],['one','two'],1,1,async()=>({pixels:[0,0,0,0],cel_opacity:1}));
 const ops=planCelPaste(copied,[{layerId:'b',frameId:'two'}],{layerId:'b',frameId:'two'},['a','b','c'],['one','two','three'],1,1);
 expect(ops.map(op=>[op.layer_id,op.frame_ids[0]])).toEqual([['b','two'],['c','three']]);
 const selected=[{layerId:'b',frameId:'two'},{layerId:'c',frameId:'three'}];
 expect(planCelPaste(copied,selected,selected[1]!,['a','b','c'],['one','two','three'],1,1).map(op=>[op.layer_id,op.frame_ids[0]])).toEqual([['b','two'],['c','three']]);
 expect(()=>planCelPaste(copied,[],{layerId:'c',frameId:'three'},['a','b','c'],['one','two','three'],1,1)).toThrow('fit');
 expect(()=>planCelPaste(copied,[],{layerId:'a',frameId:'one'},['a','b'],['one','two'],2,1)).toThrow('size');
});
test('cut never clears source cels when copying fails',async()=>{
 const operations:unknown[]=[];
 const editor={state:reactive({snapshot:{active_document:'art',managed:false,document:{generation:1,width:1,height:1,alternate_view:false,layers:[{id:'a'}],frames:[{id:'one'}]}},working:false,layer:'a',frame:0}),readCel:async()=>{throw new Error('read failed');},refresh:async()=>{}} as unknown as Editor;
 const clipboard=useCelClipboard(editor,ref([{layerId:'a',frameId:'one'}]),async operation=>{operations.push(operation);},()=>{});
 await clipboard.cut();expect(operations).toHaveLength(0);expect(clipboard.canPaste.value).toBe(false);
});
test('immediate cut/paste clears the captured source before pasting into the new selection',async()=>{
 let release!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve;});const operations:any[]=[];
 const editor={state:reactive({snapshot:{active_document:'art',managed:false,document:{generation:1,width:1,height:1,alternate_view:false,layers:[{id:'a'},{id:'b'}],frames:[{id:'one'},{id:'two'}]}},working:false,layer:'a',frame:0}),readCel:async()=>{await gate;return{pixels:[255,0,0,255],cel_opacity:1};},refresh:async()=>{}} as unknown as Editor;
 const selected=ref([{layerId:'a',frameId:'one'}]);const clipboard=useCelClipboard(editor,selected,async operation=>{operations.push(operation);},()=>{});
 const cut=clipboard.cut();selected.value=[{layerId:'b',frameId:'two'}];editor.state.layer='b';editor.state.frame=1;
 const paste=clipboard.paste();release();await cut;await paste;
 expect(operations[0][0]).toMatchObject({type:'clear_cells',layer_id:'a',frame_ids:['one']});
 expect(operations[1][0]).toMatchObject({type:'paste_cells',layer_id:'b',frame_ids:['two']});
});
