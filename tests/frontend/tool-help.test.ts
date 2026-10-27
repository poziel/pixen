import {expect,test} from 'bun:test';
import {tools} from '../../src/editor/tools/raster';
import {toolInfo,toolTitle,helpParts} from '../../src/editor/tools/toolInfo';

test('selection tools put magic wand after color and drawing after lasso',()=>{
 const ids=tools.map(tool=>tool.id);
 expect(ids[ids.indexOf('color')+1]).toBe('wand');
 expect(ids[ids.indexOf('lasso')+1]).toBe('paint-select');
});
test('help titles preserve word spacing and key markup stays plain safe text',()=>{
 expect(toolTitle('Rectangular selection')).toBe('Rectangular Selection');
 expect(helpParts('Hold `Control + Shift` to subtract.')).toEqual([{text:'Hold ',key:false},{text:'Control + Shift',key:true},{text:' to subtract.',key:false}]);
 expect(helpParts('<img onerror=alert(1)>')[0].text).toBe('<img onerror=alert(1)>');
 expect(toolInfo.rectangle.detail).toContain('`Control + Shift`');
});
