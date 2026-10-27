import {test,expect} from 'bun:test';
import {selectionSummary} from '../../src/editor/timeline/selectionSummary';
test('matrix summaries consistently put layer identity before cel count',()=>{
 const layers=[{id:'a',name:'Face'},{id:'b',name:'Body'}];
 const cells=Array.from({length:5},(_,i)=>({layerId:'a',frameId:String(i)}));
 expect(selectionSummary(cells,layers)).toBe('Face · 5 cels');
 expect(selectionSummary([...cells,...cells.map(cell=>({...cell,layerId:'b'}))],layers)).toBe('2 layers · 10 cels');
 expect(selectionSummary([{layerId:'b',frameId:'0'}],layers)).toBe('Body · 1 cel');
 expect(selectionSummary([cells[0],cells[0]],layers)).toBe('Face · 1 cel');
 expect(selectionSummary([],layers)).toBe('No cels selected');
});
