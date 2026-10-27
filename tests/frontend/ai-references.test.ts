import { expect, test } from 'bun:test';
import { prepareReferences } from '../../src/services/aiReferences';

test('reference files are snapshotted as bytes, keeping their names and order', async () => {
  const result = await prepareReferences([{id:'one',name:'reference',kind:'file',files:[new File(['palette notes'],'brief.txt')]}], new AbortController().signal);
  expect(result).toEqual([{name:'brief.txt',data:btoa('palette notes')}]);
});

test('excess files and cancelled preparation never produce provider input', async () => {
  const files = Array.from({length:9},(_,i) => new File(['x'],`${i}.txt`));
  await expect(prepareReferences([{id:'many',name:'many',kind:'file',files}],new AbortController().signal)).rejects.toThrow('eight');
  const controller = new AbortController(); controller.abort();
  await expect(prepareReferences([{id:'one',name:'one',kind:'file',files:files.slice(0,1)}],controller.signal)).rejects.toThrow();
});
