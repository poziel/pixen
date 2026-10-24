import { expect, test } from 'bun:test';
import { footprint, resizedToolValue } from '../../src/editor/tools/interaction';

test('cursor footprint matches the centered square brush for odd and even sizes', () => {
  expect(footprint('pencil', {x:5,y:6}, 3)).toEqual({x:4,y:5,size:3});
  expect(footprint('eraser', {x:5,y:6}, 2)).toEqual({x:4,y:5,size:2});
  expect(footprint('bucket', {x:5,y:6}, 99)).toEqual({x:5,y:6,size:1});
  expect(footprint('pan', {x:5,y:6}, 1)).toBeNull();
});

test('wheel resizing is bounded and only applies to scalable tools', () => {
  expect(resizedToolValue('pencil', 5, -100)).toBe(6);
  expect(resizedToolValue('eraser', 1, 100)).toBe(1);
  expect(resizedToolValue('pencil', 256, -100)).toBe(256);
  expect(resizedToolValue('text', 128, -100)).toBe(128);
  expect(resizedToolValue('bucket', 1, -100)).toBeNull();
  expect(resizedToolValue('pencil', 5, 0)).toBe(5);
});
