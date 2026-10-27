import { expect, test } from 'bun:test';
import { bytesHsb, exposeColor, hsbBytes, linearGradient, linearToSrgb, srgbToLinear } from '../../src/editor/tools/colorMath';
test('sRGB and linear-light channels round trip, including transfer thresholds', () => {
  for (const value of [0,.003,.04,.04045,.5,1]) expect(linearToSrgb(srgbToLinear(value))).toBeCloseTo(value,6);
  expect(srgbToLinear(.5)).toBeCloseTo(.214041,6);
  expect(linearToSrgb(.5)).toBeCloseTo(.735357,6);
});
test('exposure uses stops in linear light, clamps to byte colors, and preserves alpha', () => {
  expect(exposeColor([128,64,32,112],0)).toEqual([128,64,32,112]);
  expect(exposeColor([128,64,32,112],1)).toEqual([176,90,47,112]);
  expect(exposeColor([255,255,255,12],8)).toEqual([255,255,255,12]);
  expect(exposeColor([0,0,0,255],8)).toEqual([0,0,0,255]);
});
test('byte conversion retains neutral hue and linear gradients use display-encoded endpoints', () => {
  const neutral=bytesHsb([128,128,128,64],210);
  expect(neutral.h).toBe(210);
  expect(hsbBytes(neutral)).toEqual([128,128,128,64]);
  const gradient=linearGradient([0,128,255,64],0);
  expect(gradient).toContain('rgb(0 128 255) 0%');
  expect(gradient).toContain('rgb(255 128 255) 100%');
  expect(gradient).toContain('rgb(188 128 255) 50%');
});
