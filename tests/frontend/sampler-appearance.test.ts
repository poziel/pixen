import { expect, test } from 'bun:test';
import { samplerBorder } from '../../src/editor/tools/samplerAppearance';

test('sampler always chooses contrasting black or white',()=>{
  expect(samplerBorder([0,0,0])).toEqual([255,255,255]);
  expect(samplerBorder([14,20,26])).toEqual([255,255,255]);
  expect(samplerBorder([228,212,202])).toEqual([0,0,0]);
  // Different hues at similar brightness still need a clear edge.
  expect(samplerBorder([20,100,20])).toEqual([255,255,255]);
});
