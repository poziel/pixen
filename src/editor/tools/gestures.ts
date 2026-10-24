import { ref, watch, onMounted, onBeforeUnmount, type Ref } from 'vue';
import type { Editor } from '../editor';
import { brush, flood, movePixels, pixelHex, rgba, selectionMask, type Point, type Tool } from './raster';
export function useTools(editor: Editor, stage: Ref<HTMLElement | undefined>, canvas: Ref<HTMLCanvasElement | undefined>, overlay: Ref<HTMLCanvasElement | undefined>, setScale: (n: number, a?: Point) => void, scale: Ref<number>) {
    const s = editor.state, t = s.tools, panHeld = ref(false);
    const heldPanKeys = new Set<string>();
    watch(panHeld, value => { t.temporaryPan = value; });
    let selection: Set<number> | null = null, polygon: Point[] = [], polygonTool: Tool = 'polygon';
    type Gesture = {
        tool: Tool;
        path: Point[];
        button: number;
        args: Record<string, unknown>;
        generation: number;
        width: number;
        height: number;
        before?: Uint8ClampedArray;
        after?: Uint8ClampedArray;
        released: boolean;
        cancelled: boolean;
        color: string;
        settings: {
            size: number;
            density: number;
            opacity: number;
        };
        composite?: ImageData;
        lower?: HTMLImageElement;
        upper?: HTMLImageElement;
        opacity?: number;
    };
    let gesture: Gesture | null = null;
    let loading = false;
    let lastPolygonClick = { time: 0, x: 0, y: 0, button: -1 };
    function clear() { selection = null; polygon = []; paintSelection(); }
    function point(e: PointerEvent | MouseEvent): Point { const b = canvas.value!.getBoundingClientRect(); return { x: Math.floor((e.clientX - b.left) / b.width * canvas.value!.width), y: Math.floor((e.clientY - b.top) / b.height * canvas.value!.height) }; }
    function paintSelection(path?: Point[], kind?: 'rectangle' | 'ellipse' | 'polygon' | 'lasso') {
        const c = overlay.value;
        if (!c || !canvas.value)
            return;
        c.width = canvas.value.width;
        c.height = canvas.value.height;
        const ctx = c.getContext('2d')!;
        const mask = path && kind ? selectionMask(kind, c.width, c.height, path) : selection;
        if (mask) {
            ctx.fillStyle = 'rgba(60,160,255,.25)';
            for (const n of mask)
                ctx.fillRect(n % c.width, Math.floor(n / c.width), 1, 1);
            ctx.strokeStyle = 'white';
            ctx.lineWidth = 1 / scale.value;
            ctx.beginPath();
            for (const n of mask) {
                const x = n % c.width, y = Math.floor(n / c.width);
                if (x === 0 || !mask.has(n - 1)) {
                    ctx.moveTo(x, y);
                    ctx.lineTo(x, y + 1);
                }
                if (x === c.width - 1 || !mask.has(n + 1)) {
                    ctx.moveTo(x + 1, y);
                    ctx.lineTo(x + 1, y + 1);
                }
                if (y === 0 || !mask.has(n - c.width)) {
                    ctx.moveTo(x, y);
                    ctx.lineTo(x + 1, y);
                }
                if (y === c.height - 1 || !mask.has(n + c.width)) {
                    ctx.moveTo(x, y + 1);
                    ctx.lineTo(x + 1, y + 1);
                }
            }
            ctx.stroke();
        }
        if (polygon.length) {
            ctx.strokeStyle = '#55aaff';
            ctx.lineWidth = 2 / scale.value;
            ctx.beginPath();
            polygon.forEach((p, i) => i ? ctx.lineTo(p.x + .5, p.y + .5) : ctx.moveTo(p.x + .5, p.y + .5));
            ctx.stroke();
        }
    }
    function select(mask: Set<number>) { if (t.selectionMode === 'replace' || !selection)
        selection = t.selectionMode === 'subtract' ? new Set() : mask;
    else if (t.selectionMode === 'add')
        for (const n of mask)
            selection.add(n);
    else
        for (const n of mask)
            selection.delete(n); paintSelection(); }
    function preview() {
        const g = gesture;
        if (!g?.before || !g.composite || !canvas.value)
            return;
        const data = g.before.slice(), last = g.path[g.path.length - 1];
        if (g.tool === 'pencil' || g.tool === 'eraser')
            brush(data, g.width, g.height, g.path, g.color, g.settings, g.tool === 'eraser', selection);
        else if (g.tool === 'move')
            movePixels(data, g.width, g.height, selection, last.x - g.path[0].x, last.y - g.path[0].y, t.moveCopy);
        g.after = data;
        const cel = document.createElement('canvas');
        cel.width = g.width;
        cel.height = g.height;
        cel.getContext('2d')!.putImageData(new ImageData(data, g.width, g.height), 0, 0);
        const ctx = canvas.value.getContext('2d')!;
        ctx.clearRect(0, 0, g.width, g.height);
        if (g.lower)
            ctx.drawImage(g.lower, 0, 0);
        ctx.globalAlpha = g.opacity ?? 1;
        ctx.drawImage(cel, 0, 0);
        ctx.globalAlpha = 1;
        if (g.upper)
            ctx.drawImage(g.upper, 0, 0);
    }
    function cancel() { const g = gesture; if (g) {
        g.cancelled = true;
        if (g.composite && canvas.value && canvas.value.width === g.width)
            canvas.value.getContext('2d')!.putImageData(g.composite, 0, 0);
    } gesture = null; loading = false; polygon = []; paintSelection(); }
    async function commit() {
        const g = gesture;
        if (!g || g.cancelled || !g.before || !g.after)
            return;
        const pixels = [];
        for (let n = 0; n < g.before.length; n += 4)
            if (g.before.subarray(n, n + 4).some((v, i) => v !== g.after![n + i]))
                pixels.push({ x: (n / 4) % g.width, y: Math.floor(n / 4 / g.width), color: pixelHex(g.after, n / 4) });
        gesture = null;
        if (pixels.length)
            await editor.action('paint_pixels', { ...g.args, generation: g.generation, pixels });
        if (s.error && g.composite && canvas.value)
            canvas.value.getContext('2d')!.putImageData(g.composite, 0, 0);
        if (g.tool === 'move' && selection && !s.error) {
            const last = g.path[g.path.length - 1], dx = last.x - g.path[0].x, dy = last.y - g.path[0].y;
            selection = new Set([...selection].flatMap(n => { const x = n % g.width + dx, y = Math.floor(n / g.width) + dy; return x >= 0 && y >= 0 && x < g.width && y < g.height ? [y * g.width + x] : []; }));
        }
        paintSelection();
    }
    function applySingle() {
        const g = gesture;
        if (!g?.before)
            return;
        const data = g.before.slice(), p = g.path[0];
        if (g.tool === 'color') {
            select(flood(data, g.width, g.height, p.x, p.y, t.tolerance, null, t.contiguous));
            gesture = null;
            return;
        }
        if (g.tool === 'eyedropper') {
            t[g.button === 2 ? 'secondary' : 'primary'] = pixelHex(data, p.y * g.width + p.x);
            gesture = null;
            return;
        }
        if (g.tool === 'bucket' || g.tool === 'paper') {
            const indices = g.tool === 'bucket' ? flood(data, g.width, g.height, p.x, p.y, t.tolerance, selection, t.contiguous) : selection ?? new Set(Array.from({ length: g.width * g.height }, (_, i) => i));
            const ink = rgba(g.color);
            for (const n of indices)
                data.set(ink, n * 4);
        }
        else if (g.tool === 'text') {
            const c = document.createElement('canvas');
            c.width = g.width;
            c.height = g.height;
            const ctx = c.getContext('2d')!;
            ctx.font = `${Math.max(1, Math.min(128, t.textSize))}px monospace`;
            ctx.textBaseline = 'top';
            ctx.fillStyle = g.color.slice(0, 7);
            t.text.split('\n').forEach((text, i) => ctx.fillText(text, p.x, p.y + i * Math.ceil(t.textSize * 1.2)));
            const text = ctx.getImageData(0, 0, g.width, g.height).data;
            for (let n = 0; n < text.length; n += 4)
                if (text[n + 3] >= 128 && (!selection || selection.has(n / 4)))
                    data.set([text[n], text[n + 1], text[n + 2], rgba(g.color)[3]], n);
        }
        g.after = data;
    }
    async function down(e: PointerEvent): Promise<boolean> {
        const tool = e.button === 2 ? t.right : t.left;
        if (e.button === 1 || panHeld.value || tool === 'pan')
            return false;
        if (e.button !== 0 && e.button !== 2)
            return true;
        if (!canvas.value || !s.snapshot?.document)
            return true;
        stage.value?.focus({ preventScroll: true });
        const p = point(e), doc = s.snapshot.document;
        if (tool === 'zoom') {
            const b = stage.value!.getBoundingClientRect();
            setScale(scale.value * (e.button === 2 || e.altKey ? .8 : 1.25), { x: e.clientX - b.left - b.width / 2, y: e.clientY - b.top - b.height / 2 });
            return true;
        }
        if (gesture || loading || s.working)
            return true;
        if (p.x < 0 || p.y < 0 || p.x >= doc.width || p.y >= doc.height)
            return true;
        if (['rectangle', 'ellipse', 'polygon', 'lasso'].includes(tool)) {
            s.playing = false;
            if (tool === 'polygon') {
                polygonTool = tool;
                const double = e.timeStamp - lastPolygonClick.time < 400 && e.button === lastPolygonClick.button && Math.hypot(e.clientX - lastPolygonClick.x, e.clientY - lastPolygonClick.y) < 5;
                lastPolygonClick = { time: e.timeStamp, x: e.clientX, y: e.clientY, button: e.button };
                if (double && polygon.length >= 3) {
                    finishPolygon();
                    return true;
                }
                polygon.push(p);
                paintSelection();
                return true;
            }
            stage.value?.setPointerCapture(e.pointerId);
            gesture = { tool, path: [p], button: e.button, args: {}, generation: doc.generation, width: doc.width, height: doc.height, released: false, cancelled: false, color: '', settings: { size: 1, density: 100, opacity: 100 } };
            return true;
        }
        if (s.snapshot.managed || doc.alternate_view || s.sheet || s.mode !== 'composite' || s.representation !== 'color' || s.previewSize) {
            s.error = 'Switch to the original color artwork to edit. Managed previews are read-only.';
            return true;
        }
        if (!s.layer || s.hidden.includes(s.layer) || (s.isolatedLayer && s.isolatedLayer !== s.layer)) {
            s.error = 'Select a visible layer before editing.';
            return true;
        }
        s.playing = false;
        stage.value?.setPointerCapture(e.pointerId);
        const args = { documentId: s.snapshot.active_document, frameId: doc.frames[s.frame].id, layerId: s.layer };
        const slot = e.button === 2 ? 'right' : 'left';
        const g: Gesture = gesture = { tool, path: [p], button: e.button, args, generation: doc.generation, width: doc.width, height: doc.height, released: false, cancelled: false, color: e.button === 2 ? t.secondary : t.primary, settings: { ...t.brushes[slot][tool === 'eraser' ? 'eraser' : 'pencil'] }, composite: canvas.value.getContext('2d')!.getImageData(0, 0, doc.width, doc.height) };
        loading = true;
        try {
            const request = editor.request()!, index = doc.layers.findIndex(l => l.id === s.layer);
            async function part(above: boolean) {
                const hidden_layers = doc.layers.filter((l, i) => s.hidden.includes(l.id) || (s.isolatedLayer && s.isolatedLayer !== l.id) || (above ? i <= index : i >= index)).map(l => l.id);
                const image = await editor.image({ ...request, layer_id: null, hidden_layers, annotations: false });
                const bitmap = new Image();
                bitmap.src = `data:image/png;base64,${image.png}`;
                await bitmap.decode();
                return bitmap;
            }
            const [cel, lower, upper] = await Promise.all([editor.readCel(args), part(false), part(true)]);
            loading = false;
            if (g.cancelled || gesture !== g)
                return true;
            if (s.snapshot?.document?.generation !== g.generation) {
                cancel();
                s.error = 'Artwork changed; retry the stroke.';
                return true;
            }
            g.before = new Uint8ClampedArray(cel.pixels);
            g.opacity = cel.opacity;
            g.lower = lower;
            g.upper = upper;
            if (['pencil', 'eraser', 'move'].includes(tool))
                preview();
            else
                applySingle();
            if (g.released && gesture === g)
                await commit();
        }
        catch (error) {
            loading = false;
            cancel();
            s.error = String(error);
        }
        return true;
    }
    function move(e: PointerEvent) {
        const g = gesture;
        if (!g || g.released || !canvas.value)
            return;
        const p = point(e);
        if (g.path.at(-1)?.x === p.x && g.path.at(-1)?.y === p.y)
            return;
        g.path.push(p);
        if (['rectangle', 'ellipse', 'lasso'].includes(g.tool))
            paintSelection(g.path, g.tool as 'rectangle' | 'ellipse' | 'lasso');
        else if (['pencil', 'eraser', 'move'].includes(g.tool))
            preview();
    }
    function up() {
        const g = gesture;
        if (!g)
            return;
        g.released = true;
        if (['rectangle', 'ellipse', 'lasso'].includes(g.tool)) {
            select(selectionMask(g.tool as 'rectangle' | 'ellipse' | 'lasso', g.width, g.height, g.path));
            gesture = null;
        }
        else if (g.before)
            void commit();
    }
    function finishPolygon() { if (polygon.length >= 3 && canvas.value) {
        select(selectionMask(polygonTool as 'polygon', canvas.value.width, canvas.value.height, polygon));
        polygon = [];
        paintSelection();
    } }
    function doubleClick() { return t.left === 'polygon' || t.right === 'polygon'; }
    function key(e: KeyboardEvent) {
        if (s.dialog || e.defaultPrevented || (e.target as HTMLElement)?.closest('input,textarea,select,[contenteditable=true],[role=dialog]'))
            return;
        if ((e.code === 'Space' || e.key === 'Enter') && !e.ctrlKey && !e.metaKey && !e.altKey && !(e.target as HTMLElement)?.closest('button')) {
            heldPanKeys.add(e.code);
            panHeld.value = true;
            e.preventDefault();
        }
        if (e.key === 'Escape') {
            cancel();
            clear();
            e.preventDefault();
        }
        if (e.key.toLowerCase() === 'x' && !e.ctrlKey && !e.metaKey) {
            [t.primary, t.secondary] = [t.secondary, t.primary];
            e.preventDefault();
        }
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z' && !s.working) {
            cancel();
            void editor.action('edit_history', { documentId: s.snapshot?.active_document, redo: e.shiftKey });
            e.preventDefault();
        }
    }
    function keyup(e: KeyboardEvent) {
        heldPanKeys.delete(e.code);
        panHeld.value = heldPanKeys.size > 0;
    }
    function blur() { heldPanKeys.clear(); panHeld.value = false; cancel(); }
    watch([() => s.snapshot?.active_document, () => s.frame, () => s.layer, () => s.sheet, () => s.mode, () => s.representation], () => { cancel(); clear(); });
    watch(() => s.selectionClear, clear);
    watch(scale, () => paintSelection());
    onMounted(() => { window.addEventListener('keydown', key); window.addEventListener('keyup', keyup); window.addEventListener('blur', blur); });
    onBeforeUnmount(() => { cancel(); t.temporaryPan=false; window.removeEventListener('keydown', key); window.removeEventListener('keyup', keyup); window.removeEventListener('blur', blur); });
    return { down, move, up, cancel, doubleClick, panHeld, paintSelection };
}
