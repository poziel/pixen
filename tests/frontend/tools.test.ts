import { describe, expect, test } from 'bun:test';
import { brush, flood, selectionMask, movePixels, hsvToHex, hexToHsv } from '../../src/editor/tools/raster';
describe('manual pixel tools', () => {
    test('brush interpolates, clips to selection, and applies opacity once per stroke', () => {
        const data = new Uint8ClampedArray(5 * 4);
        const selection = new Set([1, 2, 3]);
        brush(data, 5, 1, [{ x: 0, y: 0 }, { x: 4, y: 0 }], '#ff0000', { size: 1, density: 100, opacity: 50 }, false, selection);
        expect([...data.filter((_, i) => i % 4 === 3)]).toEqual([0, 128, 128, 128, 0]);
    });
    test('flood respects connectivity and selection boundaries', () => {
        const data = new Uint8ClampedArray([0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0]);
        expect([...flood(data, 3, 1, 0, 0, 0)]).toEqual([0]);
    });
    test('ellipse and polygon selections use pixel centers', () => {
        expect(selectionMask('ellipse', 3, 3, [{ x: 0, y: 0 }, { x: 2, y: 2 }]).size).toBe(9);
        expect(selectionMask('polygon', 4, 4, [{ x: 0, y: 0 }, { x: 4, y: 0 }, { x: 0, y: 4 }]).has(15)).toBe(false);
    });
    test('move uses a snapshot, clears source, and clips at edges', () => {
        const data = new Uint8ClampedArray([255, 0, 0, 255, 0, 0, 255, 255, 0, 0, 0, 0]);
        movePixels(data, 3, 1, new Set([0, 1]), 1, 0, false);
        expect([...data]).toEqual([0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 255, 255]);
    });
    test('HSV preserves primary colors', () => {
        expect(hsvToHex(120, 1, 1)).toBe('#00ff00');
        expect(hexToHsv('#0000ff')).toEqual([240, 1, 1]);
    });
    test('density zero paints nothing and eraser opacity preserves RGB until transparent', () => {
        const data = new Uint8ClampedArray([255, 0, 0, 255]);
        brush(data, 1, 1, [{x:0,y:0}], '#0000ff', {size:1,density:0,opacity:100});
        expect([...data]).toEqual([255,0,0,255]);
        brush(data, 1, 1, [{x:0,y:0}], '#ffffff', {size:1,density:100,opacity:50}, true);
        expect([...data]).toEqual([255,0,0,128]);
    });
    test('freehand selection closes its path and excludes outside pixels', () => {
        const mask=selectionMask('lasso',4,4,[{x:0,y:0},{x:3,y:0},{x:3,y:3},{x:0,y:3}]);
        expect(mask.size).toBe(9);
        expect(mask.has(15)).toBe(false);
    });
});
