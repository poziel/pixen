export type Point = {
    x: number;
    y: number;
};
export type Tool = 'rectangle' | 'ellipse' | 'polygon' | 'color' | 'lasso' | 'move' | 'zoom' | 'pan' | 'text' | 'paper' | 'pencil' | 'eraser' | 'bucket' | 'eyedropper' | 'line' | 'curve' | 'draw-rectangle' | 'draw-ellipse' | 'isometric' | 'paint-select' | 'wand';
export type Brush = {
    size: number;
    density: number;
    opacity: number;
    shape?: 'square'|'circle';
    filled?: boolean;
    fillInside?: boolean;
    overwrite?: boolean;
    spacing?: boolean;
    gapX?: number;
    gapY?: number;
};
export const tools: {
    id: Tool;
    label: string;
}[] = [
    { id: 'rectangle', label: 'Rectangular Selection' }, { id: 'ellipse', label: 'Elliptical Selection' },
    { id: 'polygon', label: 'Polygonal Selection' }, { id: 'color', label: 'Select By Color' },
    { id:'wand',label: 'Magic Wand' },
    { id: 'lasso', label: 'Lasso / Free Select' }, { id:'paint-select',label: 'Select By Drawing' }, { id: 'move', label: 'Move' },
    { id: 'zoom', label: 'Zoom' }, { id: 'pan', label: 'Pan' }, { id: 'text', label: 'Text' },
    { id: 'paper', label: 'Colored Paper' }, { id: 'pencil', label: 'Pencil' },
    { id: 'eraser', label: 'Eraser' }, { id: 'bucket', label: 'Bucket' }, { id: 'eyedropper', label: 'Color Picker' },
    {id:'line',label: 'Line'},{id:'curve',label: 'Curve'},{id:'draw-rectangle',label: 'Rectangle'},{id:'draw-ellipse',label: 'Ellipse'},{id:'isometric',label: 'Isometric Box'},
];
export function hsvToHex(h: number, s: number, v: number): string {
    const c = v * s, x = c * (1 - Math.abs((h / 60) % 2 - 1)), m = v - c;
    const rgb = h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
    return '#' + rgb.map(n => Math.round((n + m) * 255).toString(16).padStart(2, '0')).join('');
}
export function hexToHsv(hex: string): [
    number,
    number,
    number
] {
    const [r, g, b] = [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255), max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min;
    let h = !d ? 0 : max === r ? 60 * (((g - b) / d) % 6) : max === g ? 60 * ((b - r) / d + 2) : 60 * ((r - g) / d + 4);
    if (h < 0)
        h += 360;
    return [h, max === 0 ? 0 : d / max, max];
}
export function rgba(hex: string): number[] { return [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16)).concat(hex.length === 9 ? parseInt(hex.slice(7, 9), 16) : 255); }
export function pixelHex(data: Uint8ClampedArray, index: number): string { return '#' + [...data.slice(index * 4, index * 4 + 4)].map(n => n.toString(16).padStart(2, '0')).join(''); }
export function line(a: Point, b: Point): Point[] {
    const count = Math.max(Math.abs(b.x - a.x), Math.abs(b.y - a.y)), result: Point[] = [];
    for (let i = 0; i <= count; i++) {
        const t = count ? i / count : 0;
        result.push({ x: Math.round(a.x + (b.x - a.x) * t), y: Math.round(a.y + (b.y - a.y) * t) });
    }
    return result;
}
export function brush(data: Uint8ClampedArray, w: number, h: number, path: Point[], color: string, settings: Brush, erase = false, selection: Set<number> | null = null, interpolate = true) {
    const ink = rgba(color), visited = new Set<number>(), size = Math.max(1, Math.min(256, Math.round(settings.size) || 1));
    const centers = interpolate ? path.flatMap((p, i) => i ? line(path[i - 1], p) : [p]) : [...path];
    if(settings.fillInside&&!erase&&path.length>=3)
        for(const n of selectionMask('polygon',w,h,path))centers.push({x:n%w,y:Math.floor(n/w)});
    let lastStamp:Point|undefined;
    for (const p of centers) {
        if(settings.spacing&&lastStamp&&Math.abs(p.x-lastStamp.x)<size+(settings.gapX??0)&&Math.abs(p.y-lastStamp.y)<size+(settings.gapY??0))continue;
        lastStamp=p;
        for (let oy = 0; oy < size; oy++)
            for (let ox = 0; ox < size; ox++) {
                const dx=ox+.5-size/2,dy=oy+.5-size/2;
                if(settings.shape==='circle'){
                    if(dx*dx+dy*dy>(size/2)**2)continue;
                    if(settings.filled===false&&size>2&&dx*dx+dy*dy<=((size-2)/2)**2)continue;
                }else if(settings.filled===false&&ox>0&&oy>0&&ox<size-1&&oy<size-1)continue;
                const x = p.x + ox - Math.floor(size / 2), y = p.y + oy - Math.floor(size / 2), n = y * w + x;
                if (x < 0 || y < 0 || x >= w || y >= h || visited.has(n) || (selection && !selection.has(n)))
                    continue;
                visited.add(n);
                if (((Math.imul(n + 1, 1103515245) >>> 0) % 10000) / 100 >= settings.density)
                    continue;
                const a = settings.opacity / 100 * (erase ? 1 : ink[3] / 255), i = n * 4;
                if (erase) {
                    data[i + 3] = Math.round(data[i + 3] * (1 - a));
                    if (!data[i + 3])
                        data.fill(0, i, i + 4);
                    continue;
                }
                if(settings.overwrite){data.set([ink[0],ink[1],ink[2],Math.round(a*255)],i);continue;}
                const old = data[i + 3] / 255, out = a + old * (1 - a);
                if (out)
                    for (let c = 0; c < 3; c++)
                        data[i + c] = (ink[c] * a + data[i + c] * old * (1 - a)) / out;
                data[i + 3] = Math.round(out * 255);
            }
    }
}
function matches(data: Uint8ClampedArray, a: number, b: number, t: number) { for (let c = 0; c < 4; c++)
    if (Math.abs(data[a * 4 + c] - data[b * 4 + c]) > t)
        return false; return true; }
export function flood(data: Uint8ClampedArray, w: number, h: number, x: number, y: number, tolerance: number, selection: Set<number> | null = null, contiguous = true): Set<number> {
    const result = new Set<number>();
    if (x < 0 || y < 0 || x >= w || y >= h)
        return result;
    const seed = y * w + x;
    if (!contiguous) {
        for (let n = 0; n < w * h; n++)
            if ((!selection || selection.has(n)) && matches(data, n, seed, tolerance))
                result.add(n);
        return result;
    }
    const queue = [seed], seen = new Set<number>();
    for (let q = 0; q < queue.length; q++) {
        const n = queue[q];
        if (seen.has(n))
            continue;
        seen.add(n);
        if ((selection && !selection.has(n)) || !matches(data, n, seed, tolerance))
            continue;
        result.add(n);
        if (n % w > 0)
            queue.push(n - 1);
        if (n % w < w - 1)
            queue.push(n + 1);
        if (n >= w)
            queue.push(n - w);
        if (n < w * (h - 1))
            queue.push(n + w);
    }
    return result;
}
export function selectionMask(kind: 'rectangle' | 'ellipse' | 'polygon' | 'lasso', w: number, h: number, path: Point[]): Set<number> {
    const result = new Set<number>();
    if (!path.length)
        return result;
    const a = path[0], b = path[path.length - 1], left = Math.min(a.x, b.x), top = Math.min(a.y, b.y), rw = Math.abs(a.x - b.x) + 1, rh = Math.abs(a.y - b.y) + 1;
    for (let y = 0; y < h; y++)
        for (let x = 0; x < w; x++) {
            let inside = false;
            if (kind === 'rectangle')
                inside = x >= left && x < left + rw && y >= top && y < top + rh;
            else if (kind === 'ellipse')
                inside = ((x + .5 - left - rw / 2) / (rw / 2)) ** 2 + ((y + .5 - top - rh / 2) / (rh / 2)) ** 2 <= 1;
            else
                for (let i = 0, j = path.length - 1; i < path.length; j = i++) {
                    const p = path[i], q = path[j];
                    if ((p.y > y + .5) !== (q.y > y + .5) && x + .5 < (q.x - p.x) * (y + .5 - p.y) / (q.y - p.y) + p.x)
                        inside = !inside;
                }
            if (inside)
                result.add(y * w + x);
        }
    return result;
}
export function movePixels(data: Uint8ClampedArray, w: number, h: number, selection: Set<number> | null, dx: number, dy: number, copy: boolean) {
    const original = data.slice(), indices = selection ?? new Set(Array.from({ length: w * h }, (_, i) => i));
    if (!copy)
        for (const n of indices)
            data.fill(0, n * 4, n * 4 + 4);
    for (const n of indices) {
        const x = n % w + dx, y = Math.floor(n / w) + dy;
        if (x >= 0 && y >= 0 && x < w && y < h)
            data.set(original.slice(n * 4, n * 4 + 4), (y * w + x) * 4);
    }
}
