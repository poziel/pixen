import { isSelectionTool, toolForKey } from './toolInfo';
import { selectionBrushMask, pickedChannels } from './toolPixels';
import { drawToolText } from './textDrawing';
import type { ToolOptions } from './toolOptions';
import { ShapeDraft, isShape, shapePoints } from './shapes';
import { PolygonTrace } from './polygonTrace';
import {inspectionPreview} from './inspectionPreview';
import { placementPath, beginModifiers, updateModifiers, type PlacementModifiers, boundaryPath, maskBounds, combineSelection, selectionMode, resizeBounds, transformMask, handlePoint, selectionHandles, type SelectionBounds, type SelectionHandle, type SelectionMode } from './selection';
import { rememberColor } from './recent';
import { ref, watch, onMounted, onBeforeUnmount, type Ref } from 'vue';
import type { Editor } from '../editor';
import { brush, flood, movePixels, pixelHex, rgba, selectionMask, type Brush, type Point, type Tool } from './raster';
export function useTools(editor: Editor, stage: Ref<HTMLElement | undefined>, canvas: Ref<HTMLCanvasElement | undefined>, setScale: (n: number, a?: Point) => void, scale: Ref<number>) {
    const s = editor.state, t = s.tools, panHeld = ref(false);
    const heldPanKeys = new Set<string>();
    const selectionCursor = ref<string>();
    watch(panHeld, value => { t.temporaryPan = value; });
    let selection: Set<number> | null = null;
    const trace=new PolygonTrace();
    type Gesture = {
        mode?: SelectionMode;
        options?:ToolOptions;
        straight?:boolean;
        modifiers?: PlacementModifiers;
        draft?: ShapeDraft;
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
        settings: Brush;
        composite?: ImageData;
        originalView?:ImageData;
        inspection?:{mode:'onion_skin'|'frame_comparison';previous?:Uint8ClampedArray;next?:Uint8ClampedArray};
        lower?: HTMLImageElement;
        upper?: HTMLImageElement;
        opacity?: number;
    };
    let gesture: Gesture | null = null;
    let loading = false;
    const lastBrushPoints=new Map<string,Point>();
    let boundaryPreview: {mask:Set<number>;from:SelectionBounds;to:SelectionBounds}|null=null;
    function clear() { boundaryPreview=null;transform=null;selection = null; trace.reset(); s.selectionView.guides=false;paintSelection(); }
    function point(e: PointerEvent | MouseEvent): Point { const b = canvas.value!.getBoundingClientRect(); return { x: Math.floor((e.clientX - b.left) / b.width * canvas.value!.width), y: Math.floor((e.clientY - b.top) / b.height * canvas.value!.height) }; }
    let polygonMode: SelectionMode = 'replace';
    let polygonModifiers: PlacementModifiers | null = null;
    function constrained(path:Point[],kind:'rectangle'|'ellipse'|'polygon'|'lasso',modifiers?:PlacementModifiers|null){
        return placementPath(path,kind,modifiers?.proportional??false,modifiers?.centered??false);
    }
    function placementModifiers(shift:boolean,control:boolean){
        if(gesture?.modifiers&&!gesture.released&&['rectangle','ellipse','lasso'].includes(gesture.tool)){
            updateModifiers(gesture.modifiers,shift,control);
            paintSelection(gesture.path,gesture.tool as 'rectangle'|'ellipse'|'lasso');
        }
        if(trace.points.length&&polygonModifiers){updateModifiers(polygonModifiers,shift,control);paintSelection();}
    }
    let transform: { mask: Set<number>; bounds: SelectionBounds; start: Point; handle: SelectionHandle | 'move'; pointerId: number } | null = null;
    function paintSelection(path?: Point[], kind?: 'rectangle' | 'ellipse' | 'polygon' | 'lasso', shown?: Set<number>) {
        if (!canvas.value) return;
        const width=canvas.value.width;
        let mask=shown ?? selection;
        if(path&&kind) mask=combineSelection(selection,selectionMask(kind,width,canvas.value.height,constrained(path,kind,gesture?.modifiers)),gesture?.mode??t.selectionMode);
        s.selectionView.bounds=maskBounds(mask,width);
        s.selectionView.outline=boundaryPath(mask,width);
        if(boundaryPreview){s.selectionView.bounds=boundaryPreview.to;s.selectionView.outline=boundaryPath(boundaryPreview.mask,width,boundaryPreview);}
        if(path&&kind){
            const raw=constrained(path,kind,gesture?.modifiers);
            if(raw.some(p=>p.x<0||p.y<0||p.x>=width||p.y>=canvas.value!.height)){
                let left=Infinity,top=Infinity,right=-Infinity,bottom=-Infinity;
                for(const p of raw){left=Math.min(left,p.x);top=Math.min(top,p.y);right=Math.max(right,p.x+1);bottom=Math.max(bottom,p.y+1);}
                const w=right-left,h=bottom-top;
                const outline=kind==='rectangle'?`M${left} ${top}h${w}v${h}h${-w}Z`:kind==='ellipse'?`M${left} ${top+h/2}a${w/2} ${h/2} 0 1 0 ${w} 0a${w/2} ${h/2} 0 1 0 ${-w} 0`:raw.map((p,i)=>`${i?'L':'M'}${p.x+.5} ${p.y+.5}`).join('')+'Z';
                if(gesture?.mode==='replace'){s.selectionView.bounds={x:left,y:top,width:w,height:h};s.selectionView.outline=outline;}
                else s.selectionView.outline+=outline;
            }
        }
        const traced=constrained(trace.preview,'polygon',polygonModifiers);
        s.selectionView.polygon=traced.map((p,i)=>`${i?'L':'M'}${p.x+.5} ${p.y+.5}`).join('')+(trace.points.length>=2?'Z':'');
        s.selectionView.polygonStart=traced[0]??null;
        s.selectionView.polygonClosable=trace.closable;
    }
    function select(mask: Set<number>, mode: SelectionMode = t.selectionMode) {
        selection=combineSelection(selection,mask,mode);
        paintSelection();
    }
    function transformSelection(e: PointerEvent) {
        if(!transform||!canvas.value)return;
        const p=point(e), dx=p.x-transform.start.x,dy=p.y-transform.start.y;
        const bounds=transform.handle==='move'?{...transform.bounds,x:transform.bounds.x+dx,y:transform.bounds.y+dy}:
            resizeBounds(transform.bounds,transform.handle,dx,dy,e.shiftKey,e.ctrlKey||e.metaKey);
        boundaryPreview={mask:transform.mask,from:transform.bounds,to:bounds};
        selection=transformMask(transform.mask,canvas.value.width,canvas.value.height,transform.bounds,bounds);
        s.selectionView.guides=true;paintSelection();
    }
    function paintBrushSelection(g:Gesture){
        const mask=combineSelection(selection,selectionBrushMask(g.width,g.height,g.path,g.settings.size,g.settings.shape??'square'),g.mode??t.selectionMode);
        s.selectionView.bounds=maskBounds(mask,g.width);s.selectionView.outline=boundaryPath(mask,g.width);
    }
    function preview() {
        const g = gesture;
        if (!g?.before || !g.composite || !canvas.value)
            return;
        const data = g.before.slice(), last = g.path[g.path.length - 1];
        if(g.draft){
            s.shapeAnchors=g.draft.staged?g.draft.points.map(p=>({...p})):[];
            const points=shapePoints(g.draft.tool,g.draft.preview,g.draft.tangents,g.draft.modifiers.shift,g.draft.modifiers.ctrl,g.width,g.height);
            brush(data,g.width,g.height,points,g.color,g.settings,false,selection,false);
            s.shapeGuide=g.draft.points.map((p,i)=>{const t=g.draft!.tangents[i];return t&&(t.x||t.y)?`M${p.x-t.x+.5} ${p.y-t.y+.5}L${p.x+t.x+.5} ${p.y+t.y+.5}`:'';}).join('');
        }
        else if (g.tool === 'pencil' || g.tool === 'eraser')
            brush(data, g.width, g.height, g.straight?[g.path[0],last]:g.path, g.color, g.settings, g.tool === 'eraser', selection);
        else if (g.tool === 'move')
            movePixels(data, g.width, g.height, selection, last.x - g.path[0].x, last.y - g.path[0].y, g.options!.move.copy);
        g.after = data;
        if(g.tool==='move'&&selection){
            const from=maskBounds(selection,g.width);
            if(from){boundaryPreview={mask:selection,from,to:{...from,x:from.x+last.x-g.path[0].x,y:from.y+last.y-g.path[0].y}};s.selectionView.guides=true;paintSelection();}
        }
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
        if(g.inspection){const image=ctx.getImageData(0,0,g.width,g.height);image.data.set(inspectionPreview(image.data,g.inspection.mode,g.inspection.previous,g.inspection.next));ctx.putImageData(image,0,0);}
    }
    function cancel() { t.shapeDrawing=false;s.shapeGuide='';s.shapeAnchors=[];boundaryPreview=null;if(transform){selection=transform.mask;transform=null;}s.selectionView.guides=false;const g = gesture; if (g) {
        g.cancelled = true;
        if (g.composite && canvas.value && canvas.value.width === g.width)
            canvas.value.getContext('2d')!.putImageData(g.originalView??g.composite, 0, 0);
    } gesture = null; loading = false; trace.reset(); paintSelection(); }
    async function commit() {
        const g = gesture;
        if (!g || g.cancelled || !g.before || !g.after)
            return;
        const pixels = [];
        for (let n = 0; n < g.before.length; n += 4)
            if (g.before.subarray(n, n + 4).some((v, i) => v !== g.after![n + i]))
                pixels.push({ x: (n / 4) % g.width, y: Math.floor(n / 4 / g.width), color: pixelHex(g.after, n / 4) });
        gesture = null;t.shapeDrawing=false;s.shapeGuide='';s.shapeAnchors=[];
        if (pixels.length) {
            await editor.action('paint_pixels', { ...g.args, generation: g.generation, pixels });
            if (!s.error && (['pencil','bucket','paper','text'].includes(g.tool)||isShape(g.tool))) rememberColor(t.recentColors,g.color);
        }
        if (s.error && g.composite && canvas.value)
            canvas.value.getContext('2d')!.putImageData(g.originalView??g.composite, 0, 0);
        if(!s.error&&['pencil','eraser'].includes(g.tool))lastBrushPoints.set(`${g.button}:${g.tool}`,{...g.path.at(-1)!});
        if (g.tool === 'move' && selection && !s.error) {
            const last = g.path[g.path.length - 1], dx = last.x - g.path[0].x, dy = last.y - g.path[0].y;
            selection = new Set([...selection].flatMap(n => { const x = n % g.width + dx, y = Math.floor(n / g.width) + dy; return x >= 0 && y >= 0 && x < g.width && y < g.height ? [y * g.width + x] : []; }));
        }
        boundaryPreview=null;s.selectionView.guides=false;paintSelection();
    }
    function applySingle() {
        const g = gesture;
        if (!g?.before)
            return;
        const data = g.before.slice(), p = g.path[0];
        if (g.tool === 'color'||g.tool==='wand') {
            const config=g.options![g.tool],source=config.merged?g.composite!.data:data;
            select(flood(source, g.width, g.height, p.x, p.y, config.tolerance, null, g.tool==='wand'),g.mode);
            gesture = null;
            return;
        }
        if (g.tool === 'eyedropper') {
            const config=g.options!.picker,target=config.target==='button'?(g.button===2?'secondary':'primary'):config.target;
            const source=config.source==='composite'?g.composite!.data:data;
            t[target]=pickedChannels(pixelHex(source,p.y*g.width+p.x),t[target],config.channels);
            gesture = null;
            return;
        }
        if (g.tool === 'bucket' || g.tool === 'paper') {
            const config=g.options!.bucket,source=config.merged?g.composite!.data:data;
            const indices=g.tool==='paper'||config.area==='selection'?(selection??new Set(Array.from({length:g.width*g.height},(_,i)=>i))):flood(source,g.width,g.height,p.x,p.y,config.tolerance,selection,config.area==='contiguous');
            const ink=g.tool==='bucket'&&config.fill==='transparent'?[0,0,0,0]:rgba(g.color);
            if(g.tool==='paper')ink[3]=Math.round(ink[3]*g.options!.paper.opacity/100);
            for(const n of indices)data.set(ink,n*4);
        }
        else if(g.tool==='text'){
            const c=document.createElement('canvas');c.width=g.width;c.height=g.height;
            drawToolText(c.getContext('2d')!,g.options!.text,p,g.color);
            const pixels=c.getContext('2d')!.getImageData(0,0,g.width,g.height).data,ink=rgba(g.color);
            for(let n=0;n<pixels.length;n+=4){
                const coverage=g.options!.text.antialias?pixels[n+3]/255:pixels[n+3]>=128?1:0;
                if(!coverage||(selection&&!selection.has(n/4)))continue;
                const alpha=Math.round(ink[3]*coverage).toString(16).padStart(2,'0');
                brush(data,g.width,g.height,[{x:(n/4)%g.width,y:Math.floor(n/4/g.width)}],g.color.slice(0,7)+alpha,{size:1,density:100,opacity:100});
            }
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
            setScale(scale.value * (t.options[e.button===2?'right':'left'].zoom==='out'?.8:1.25), { x: e.clientX - b.left - b.width / 2, y: e.clientY - b.top - b.height / 2 });
            return true;
        }
        if(gesture?.draft){
            if(e.button!==gesture.button)return true;
            const draft=gesture.draft;
            draft.modifiers={shift:e.shiftKey,ctrl:e.ctrlKey||e.metaKey,alt:e.altKey};
            const finish=draft.down(draft.tool==='isometric'&&e.shiftKey?draft.endpoint:p,e.timeStamp);
            if(finish)gesture.released=true;
            stage.value?.setPointerCapture(e.pointerId);preview();return true;
        }
        if (gesture || loading || s.working)
            return true;
        const isSelection=isSelectionTool(tool);
        const bounds=maskBounds(selection,doc.width);
        if(isSelection&&bounds&&selection&&!trace.points.length){
            const box=canvas.value.getBoundingClientRect();
            const hit=selectionHandles.find(id=>{const h=handlePoint(bounds,id);return Math.abs(e.clientX-(box.left+h.x/doc.width*box.width))<=6&&Math.abs(e.clientY-(box.top+h.y/doc.height*box.height))<=6;});
            if(hit||(['rectangle','ellipse','polygon','lasso'].includes(tool)&&!e.shiftKey&&!e.ctrlKey&&!e.metaKey&&t.selectionMode==='replace'&&p.x>=0&&p.x<doc.width&&p.y>=0&&p.y<doc.height&&selection.has(p.y*doc.width+p.x))){
                transform={mask:new Set(selection),bounds,start:p,handle:hit??'move',pointerId:e.pointerId};
                stage.value?.setPointerCapture(e.pointerId);s.selectionView.guides=true;paintSelection();return true;
            }
        }
        if (!isShape(tool)&&!['rectangle','ellipse','polygon','lasso','paint-select','pencil','eraser'].includes(tool)&&(p.x < 0 || p.y < 0 || p.x >= doc.width || p.y >= doc.height))
            return true;
        if (['rectangle', 'ellipse', 'polygon', 'lasso','paint-select'].includes(tool)) {
            s.playing = false;
            if (tool === 'polygon') {
                if(!trace.points.length){polygonMode=selectionMode(t.selectionMode,e.shiftKey,e.ctrlKey||e.metaKey);polygonModifiers=beginModifiers(e.shiftKey,e.ctrlKey||e.metaKey);}
                if(trace.place(p,e.timeStamp,Math.max(.75,7/scale.value),e.button)==='close'){
                    finishPolygon();return true;
                }
                paintSelection();
                return true;
            }
            stage.value?.setPointerCapture(e.pointerId);
            const config=t.options[e.button===2?'right':'left'].selectionBrush;
            gesture = { modifiers:beginModifiers(e.shiftKey,e.ctrlKey||e.metaKey), mode:tool==='paint-select'&&config.action==='deselect'?'subtract':selectionMode(t.selectionMode,e.shiftKey,e.ctrlKey||e.metaKey), tool, path:[p],button:e.button,args:{},generation:doc.generation,width:doc.width,height:doc.height,released:false,cancelled:false,color:'',settings:{size:config.size,shape:config.shape,density:100,opacity:100} };
            if(tool==='paint-select')paintBrushSelection(gesture);
            return true;
        }
        if (s.snapshot.managed || doc.alternate_view || s.sheet || s.representation !== 'color' || s.previewSize) {
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
        const g: Gesture = gesture = { mode:selectionMode(t.selectionMode,e.shiftKey,e.ctrlKey||e.metaKey), tool, path: [p], button: e.button, args, generation: doc.generation, width: doc.width, height: doc.height, released: false, cancelled: false, color: e.button === 2 ? t.secondary : t.primary, options:JSON.parse(JSON.stringify(t.options[slot])), settings: { ...(isShape(tool)?t.options[slot].shapes[tool]:t.brushes[slot][tool === 'eraser' ? 'eraser' : 'pencil']) }, composite: canvas.value.getContext('2d')!.getImageData(0, 0, doc.width, doc.height) };
        if(['pencil','eraser'].includes(tool)&&e.shiftKey){const previous=lastBrushPoints.get(`${e.button}:${tool}`);if(previous)g.path=[previous,p];g.straight=true;}
        if(isShape(tool)){g.draft=new ShapeDraft(tool,p,e.timeStamp);g.draft.modifiers={shift:e.shiftKey,ctrl:e.ctrlKey||e.metaKey,alt:e.altKey};t.shapeDrawing=true;}
        loading = true;
        try {
            const request = editor.request()!, index = doc.layers.findIndex(l => l.id === s.layer);
            g.originalView=g.composite;
            async function bitmap(requestOverride:Partial<typeof request>){
                const image=await editor.image({...request,...requestOverride,mode:'composite',annotations:false});const bitmap=new Image();bitmap.src=`data:image/png;base64,${image.png}`;await bitmap.decode();return bitmap;
            }
            async function part(above: boolean) {
                const hidden_layers = doc.layers.filter((l, i) => s.hidden.includes(l.id) || (s.isolatedLayer && s.isolatedLayer !== l.id) || (above ? i <= index : i >= index)).map(l => l.id);
                return bitmap({layer_id:null,hidden_layers});
            }
            async function inspection(){
                if(request.mode==='composite'||!request.mode)return undefined;
                const explicit=request.reference_frame_id;
                const previous=explicit??doc.frames[s.frame-1]?.id??(request.mode==='frame_comparison'?request.frame_id:undefined);
                const next=request.mode==='onion_skin'&&!explicit?doc.frames[s.frame+1]?.id:undefined;
                async function pixels(id:string|undefined){if(!id)return undefined;const image=await bitmap({frame_id:id});const c=document.createElement('canvas');c.width=g.width;c.height=g.height;c.getContext('2d')!.drawImage(image,0,0);return c.getContext('2d')!.getImageData(0,0,g.width,g.height).data;}
                const [prior,future]=await Promise.all([pixels(previous),pixels(next)]);return {mode:request.mode,previous:prior,next:future};
            }
            const [cel, lower, upper, display] = await Promise.all([editor.readCel(args), part(false), part(true),inspection()]);
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
            g.inspection=display;
            const plain=document.createElement('canvas');plain.width=g.width;plain.height=g.height;const plainContext=plain.getContext('2d')!;
            plainContext.drawImage(lower,0,0);const raw=document.createElement('canvas');raw.width=g.width;raw.height=g.height;raw.getContext('2d')!.putImageData(new ImageData(new Uint8ClampedArray(g.before),g.width,g.height),0,0);
            plainContext.globalAlpha=cel.opacity;plainContext.drawImage(raw,0,0);plainContext.globalAlpha=1;plainContext.drawImage(upper,0,0);g.composite=plainContext.getImageData(0,0,g.width,g.height);
            if (g.draft||['pencil', 'eraser', 'move'].includes(tool))
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
        if(gesture?.draft){gesture.draft.move(point(e),{shift:e.shiftKey,ctrl:e.ctrlKey||e.metaKey,alt:e.altKey},!!e.buttons);preview();return;}
        if(trace.points.length&&canvas.value){trace.move(point(e),Math.max(.75,7/scale.value));paintSelection();}
        if(transform){transformSelection(e);return;}
        selectionCursor.value=undefined;
        const tool=e.buttons===2?t.right:t.left;
        if(canvas.value&&selection&&isSelectionTool(tool)){
            const bounds=maskBounds(selection,canvas.value.width),box=canvas.value.getBoundingClientRect();
            if(bounds){
                const hit=selectionHandles.find(id=>{const p=handlePoint(bounds,id);return Math.abs(e.clientX-box.left-p.x*box.width/canvas.value!.width)<=6&&Math.abs(e.clientY-box.top-p.y*box.height/canvas.value!.height)<=6;});
                if(hit)selectionCursor.value=({nw:'nwse-resize',se:'nwse-resize',ne:'nesw-resize',sw:'nesw-resize',n:'ns-resize',s:'ns-resize',e:'ew-resize',w:'ew-resize'})[hit];
                else {const p=point(e);if(!e.shiftKey&&t.selectionMode==='replace'&&selection.has(p.y*canvas.value.width+p.x))selectionCursor.value='move';}
            }
        }
        const g = gesture;
        if(g?.modifiers)updateModifiers(g.modifiers,e.shiftKey,e.ctrlKey||e.metaKey);
        if (!g || g.released || !canvas.value)
            return;
        const p = point(e);
        if (g.path.at(-1)?.x === p.x && g.path.at(-1)?.y === p.y)
            return;
        g.path.push(p);
        if(g.tool==='paint-select')paintBrushSelection(g);
        else if (['rectangle', 'ellipse', 'lasso'].includes(g.tool))
            paintSelection(g.path, g.tool as 'rectangle' | 'ellipse' | 'lasso');
        else if (['pencil', 'eraser', 'move'].includes(g.tool))
            preview();
    }
    function up() {
        if(transform){boundaryPreview=null;transform=null;s.selectionView.guides=false;paintSelection();return;}
        const g = gesture;
        if (!g)
            return;
        if(g.draft){
            const finish=g.draft.up()||g.released;
            if(finish){g.released=true;preview();if(g.before)void commit();}else preview();
            return;
        }
        g.released = true;
        if(g.tool==='paint-select'){select(selectionBrushMask(g.width,g.height,g.path,g.settings.size,g.settings.shape??'square'),g.mode);gesture=null;return;}
        if (['rectangle', 'ellipse', 'lasso'].includes(g.tool)) {
            select(selectionMask(g.tool as 'rectangle' | 'ellipse' | 'lasso', g.width, g.height, constrained(g.path,g.tool as 'rectangle'|'ellipse'|'lasso',g.modifiers)),g.mode);
            gesture = null;
        }
        else if (g.before)
            void commit();
    }
    function finishPolygon() { if (trace.points.length >= 3 && canvas.value) {
        select(selectionMask('polygon', canvas.value.width, canvas.value.height, constrained(trace.points,'polygon',polygonModifiers)),polygonMode);
        trace.reset(true);
        paintSelection();
    } }
    function doubleClick() { return t.left === 'polygon' || t.right === 'polygon'; }
    function key(e: KeyboardEvent) {
        if (s.dialog || e.defaultPrevented || (e.target as HTMLElement)?.closest('input,textarea,select,[contenteditable=true],[role=combobox],[role=listbox],[role=option],[role=dialog],[role=alertdialog],[role=menu],[role=menubar]'))
            return;
        if(gesture&&['pencil','eraser'].includes(gesture.tool)&&e.key==='Shift'){gesture.straight=e.shiftKey;preview();}
        if(gesture?.draft&&['Shift','Control','Meta','Alt'].includes(e.key)){
            if((e.key==='Control'||e.key==='Meta')&&!e.repeat&&gesture.draft.staged){
                if(gesture.draft.points.length<=1){cancel();e.preventDefault();return;}
                gesture.draft.removeLast();
            }
            gesture.draft.modifiers={shift:e.shiftKey,ctrl:e.ctrlKey||e.metaKey,alt:e.altKey};preview();e.preventDefault();
        }
        if(['Shift','Control','Meta'].includes(e.key))placementModifiers(e.shiftKey,e.ctrlKey||e.metaKey);
        if((e.ctrlKey||e.metaKey)&&['a','d'].includes(e.key.toLowerCase())&&canvas.value&&s.snapshot?.document){
            cancel();
            if(e.key.toLowerCase()==='d')clear();
            else { selection=new Set(Array.from({length:canvas.value.width*canvas.value.height},(_,i)=>i));paintSelection(); }
            e.preventDefault();return;
        }
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
        const shapeShortcut=toolForKey(e.key);
        if(shapeShortcut&&!e.ctrlKey&&!e.metaKey&&!gesture){t[e.altKey?'right':'left']=shapeShortcut;t.temporaryPicker=false;e.preventDefault();}
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
        if(gesture?.draft&&['Shift','Control','Meta','Alt'].includes(e.key)){
            gesture.draft.modifiers={shift:e.shiftKey,ctrl:e.ctrlKey||e.metaKey,alt:e.altKey};preview();
        }
        if(['Shift','Control','Meta'].includes(e.key))placementModifiers(e.shiftKey,e.ctrlKey||e.metaKey);
        heldPanKeys.delete(e.code);
        panHeld.value = heldPanKeys.size > 0;
    }
    function blur() { heldPanKeys.clear(); panHeld.value = false; cancel(); }
    watch([() => s.snapshot?.active_document, () => s.frame, () => s.layer, () => s.sheet, () => s.mode, () => s.representation], () => { cancel(); clear();lastBrushPoints.clear(); });
    watch(() => s.selectionClear, () => {cancel();clear();});
    watch(() => s.selectionEdit.serial, () => {
        const target=s.selectionEdit.bounds,from=canvas.value&&maskBounds(selection,canvas.value.width);
        if(!target||!from||!selection||!canvas.value)return;
        cancel();selection=transformMask(selection,canvas.value.width,canvas.value.height,from,target);paintSelection();
    });
    watch([()=>t.left,()=>t.right],()=>{if(gesture?.draft&&t[gesture.button===2?'right':'left']!==gesture.tool)cancel();});
    watch([()=>t.globalPicker,()=>t.temporaryPicker],()=>{if(gesture?.draft&&(t.globalPicker||t.temporaryPicker))cancel();});
    watch(scale, () => paintSelection());
    onMounted(() => { window.addEventListener('keydown', key); window.addEventListener('keyup', keyup); window.addEventListener('blur', blur); });
    onBeforeUnmount(() => { cancel(); t.temporaryPan=false; window.removeEventListener('keydown', key); window.removeEventListener('keyup', keyup); window.removeEventListener('blur', blur); });
    return { down, move, up, cancel, doubleClick, panHeld, paintSelection, selectionCursor };
}
