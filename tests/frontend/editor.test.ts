import { expect, test } from "bun:test";
import { createEditor } from "../../src/editor/editor";
import type { Snapshot } from "../../src/services/backend";

test("manual saving does not serialize editing behind the file write", async () => {
  let release!: (value: boolean) => void;
  let started!: () => void;
  const saving = new Promise<void>(resolve => started = resolve);
  const calls: string[] = [];
  const next = snapshot(); next.managed = false; next.activity = null;
  const transport = async <T>(command: string): Promise<T> => {
    calls.push(command);
    if (command === 'snapshot') return structuredClone(next) as T;
    if (command === 'save_document') { started(); return await new Promise<boolean>(resolve => release = resolve) as T; }
    return true as T;
  };
  const editor = createEditor(transport); await editor.refresh();
  const save = editor.action('save_document', { saveAs: false });
  await saving;
  expect(editor.state.saving).toBe(true);
  expect(editor.state.working).toBe(false);
  await editor.action('paint_pixels', {});
  expect(calls).toContain('paint_pixels');
  expect(editor.state.saving).toBe(true);
  release(true); await save;
  expect(editor.state.saving).toBe(false);
  expect(editor.state.saveNotice).toBe(1);
});

test("save failure releases saving state and preserves edit availability", async () => {
  const transport = async <T>(command: string): Promise<T> => {
    if (command === 'save_document') throw new Error('disk full');
    return structuredClone(snapshot()) as T;
  };
  const editor = createEditor(transport); await editor.refresh();
  await editor.action('save_document', {});
  expect(editor.state.saving).toBe(false);
  expect(editor.state.working).toBe(false);
  expect(editor.state.error).toContain('disk full');
  expect(editor.state.saveNotice).toBe(0);
});

function snapshot(): Snapshot {
  return {
    managed: true,
    event_serial: 1,
    review_stale: false,
    review: null,
    inspection: null,
    activity: {
      label: "Drawing",
      busy: true,
      frame_id: "second",
      layer_id: null,
    },
    document: {
      generation: 1,
      revision: 0,
      width: 4,
      height: 4,
      source_width: 4,
      source_height: 4,
      alternate_view: false,
      can_toggle_view: false,
      path: "test.pixloom",
      dirty: false,
      frames: [{ id: "first", duration_ms: 100 }],
      layers: [
        { id: "a", name: "A", visible: true, opacity: 1 },
        { id: "b", name: "B", visible: true, opacity: 1 },
      ],
    },
  };
}
test("follow retries the same event when its target arrives in a later snapshot", async () => {
  let next = snapshot();
  const transport = async <T>(command: string): Promise<T> =>
    structuredClone(command === "snapshot" ? next : "missing") as T;
  const editor = createEditor(transport);
  await editor.refresh();
  next.document!.generation = 2;
  next.document!.frames.push({ id: "second", duration_ms: 100 });
  await editor.refresh();
  expect(editor.state.frame).toBe(1);
});

test("comparison keeps its event pending until the reference frame exists", async () => {
  const next = snapshot();
  next.activity = null;
  next.inspection = {
    frame_id: "first",
    reference_frame_id: "second",
    mode: "frame_comparison",
    layer_id: null,
  };
  const transport = async <T>(command: string): Promise<T> =>
    structuredClone(command === "snapshot" ? next : "missing") as T;
  const editor = createEditor(transport);
  await editor.refresh();
  expect(editor.state.reference).toBeNull();
  next.document!.generation++;
  await editor.refresh();
  next.document!.generation++;
  next.document!.frames.push({ id: "second", duration_ms: 100 });
  await editor.refresh();
  expect(editor.state.reference).toBe("second");
});
test("review replaces inspection scope and invalidates annotated renders without a document edit", async () => {
  const next = snapshot();
  next.activity = null;
  next.review = {
    report: { kind: "sprite_analysis", frame_id: "first", layer_id: "b" },
    images: [],
  };
  const calls: { command: string; args?: Record<string, unknown> }[] = [];
  const transport = async <T>(
    command: string,
    args?: Record<string, unknown>,
  ): Promise<T> => {
    calls.push({ command, args });
    return structuredClone(
      command === "snapshot"
        ? next
        : command === "render"
          ? { width: 4, height: 4, png: "test" }
          : "found",
    ) as T;
  };
  const editor = createEditor(transport);
  editor.state.mode = "frame_comparison";
  editor.state.reference = "old";
  editor.state.isolatedLayer = "a";
  editor.state.annotations = true;
  await editor.refresh();
  expect(editor.state.isolatedLayer).toBe("b");
  expect(String(editor.state.mode)).toBe("composite");
  expect(editor.state.reference).toBeNull();
  const before = editor.request()!;
  await editor.image(before);
  next.event_serial++;
  await editor.refresh();
  const after = editor.request()!;
  await editor.image(after);
  expect(JSON.stringify(after)).not.toBe(JSON.stringify(before));
  expect(calls.filter((c) => c.command === "render")).toHaveLength(2);
  await editor.review("analyze");
  expect(
    calls.find((c) => c.command === "review_document")?.args?.layerId,
  ).toBe("b");
});

test("document switching restores independent view state even for untitled documents with equal generations", async () => {
  const first = snapshot();
  first.managed = false;
  first.active_document = 'first';
  first.document!.path = null;
  first.document!.frames.push({id:'second',duration_ms:100});
  first.documents = [{id:'first',path:null,dirty:true},{id:'other',path:null,dirty:true}];
  const second = structuredClone(first);
  second.active_document = 'other';
  second.document!.width = 16;
  let next = first;
  const renders: unknown[] = [];
  const editor = createEditor(async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
    if (command === 'render') { renders.push(args?.documentId); return {width:4,height:4,png:'image'} as T; }
    return structuredClone(next) as T;
  });
  await editor.refresh();
  editor.state.frame = 1; editor.state.hidden = ['b']; editor.state.fps = 15;
  editor.state.canvasView = {zoom:3,pan:{x:40,y:-30},fit:false};
  await editor.image(editor.request()!);
  next = second; await editor.refresh();
  expect(editor.state.canvasView.zoom).toBe(1); expect(editor.state.hidden).toEqual([]);
  editor.state.canvasView.zoom = 5;
  await editor.image(editor.request()!);
  next = first; await editor.refresh();
  expect(editor.state.frame).toBe(1); expect(editor.state.hidden).toEqual(['b']); expect(editor.state.fps).toBe(15);
  expect(editor.state.canvasView).toEqual({zoom:3,pan:{x:40,y:-30},fit:false});
  expect(renders).toEqual(['first','other']);
});

test("new artwork resets tools and colors while refresh and existing tabs preserve choices", async () => {
  let next = snapshot();
  next.active_document = 'first';
  next.documents = [{id:'first',path:null,dirty:false},{id:'second',path:null,dirty:false}];
  const editor = createEditor(async <T>() => structuredClone(next) as T);
  await editor.refresh();
  const defaults = () => ({left:editor.state.tools.left,right:editor.state.tools.right,primary:editor.state.tools.primary,secondary:editor.state.tools.secondary});
  expect(defaults()).toEqual({left:'pencil',right:'eraser',primary:'#ffffff',secondary:'#000000'});
  Object.assign(editor.state.tools,{left:'line',right:'bucket',primary:'#123456',secondary:'#abcdef'});
  await editor.refresh();
  expect(defaults()).toEqual({left:'line',right:'bucket',primary:'#123456',secondary:'#abcdef'});
  next = {...next,active_document:'second'};
  await editor.refresh();
  expect(defaults()).toEqual({left:'pencil',right:'eraser',primary:'#ffffff',secondary:'#000000'});
  editor.state.tools.left = 'curve';
  next = {...next,active_document:'first'};
  await editor.refresh();
  expect(editor.state.tools.left).toBe('curve');
});

test("document operations queue rapid successive opens instead of dropping them", async () => {
  let next = snapshot(); next.managed = false;
  const opened: string[] = [];
  const editor = createEditor(async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
    if (command === 'open_document') {
      await Promise.resolve(); opened.push(String(args?.path));
      next = {...next, active_document: String(args?.path), documents: opened.map(id=>({id,path:id,dirty:false}))};
      return true as T;
    }
    return structuredClone(next) as T;
  });
  await Promise.all([editor.action('open_document',{path:'a'}),editor.action('open_document',{path:'b'})]);
  expect(opened).toEqual(['a','b']); expect(editor.state.snapshot?.active_document).toBe('b');
});

test("preview uses selected FPS and supports reverse wrap without changing document timing", async () => {
  const next = snapshot();
  next.document!.frames.push({ id: "second", duration_ms: 900 }, { id: "third", duration_ms: 50 });
  const editor = createEditor(async <T>() => structuredClone(next) as T);
  await editor.refresh();
  editor.state.follow = false;
  editor.state.playing = true;
  editor.state.frame = 0;
  editor.tick(200);
  expect(editor.state.frame).toBe(1);
  editor.state.fps = 0.5;
  editor.state.elapsed = 0;
  editor.tick(1999);
  expect(editor.state.frame).toBe(1);
  editor.tick(1);
  expect(editor.state.frame).toBe(2);
  editor.state.direction = -1;
  editor.state.fps = 5;
  editor.state.frame = 0;
  editor.tick(200);
  expect(editor.state.frame).toBe(2);
  expect(editor.state.snapshot!.document!.frames.map(f => f.duration_ms)).toEqual([100, 900, 50]);
});
test('frame-timing preview advances using stored durations rather than an FPS override',async()=>{
 const next=snapshot();next.document!.frames.push({id:'second',duration_ms:50});
 const editor=createEditor(async<T>()=>structuredClone(next) as T);await editor.refresh();
 expect(editor.state.fps).toBe(0);editor.state.follow=false;editor.state.playing=true;editor.state.frame=0;
 editor.tick(99);expect(editor.state.frame).toBe(0);editor.tick(1);expect(editor.state.frame).toBe(1);
 editor.tick(50);expect(editor.state.frame).toBe(0);
});

test('new artworks use current defaults while existing tabs keep their independent preview state', async () => {
  let next = snapshot();
  next.managed = false; next.active_document = 'first';
  next.documents = ['first','second','third'].map(id => ({id,path:null,dirty:false}));
  const editor = createEditor(async <T>() => structuredClone(next) as T);
  editor.state.defaults.background = 'black'; editor.state.defaults.fps = 8;
  await editor.refresh();
  expect(editor.state.background).toBe('black'); expect(editor.state.fps).toBe(8);
  editor.state.background = 'white'; editor.state.fps = 15; editor.state.representation = 'grayscale';
  expect(editor.state.defaults).toEqual({background:'black',fps:8});
  next = {...next,active_document:'second'}; await editor.refresh();
  expect(editor.state.background).toBe('black'); expect(editor.state.fps).toBe(8);
  expect(String(editor.state.representation)).toBe('color');
  editor.state.defaults.background = 'transparent'; editor.state.defaults.fps = 5;
  expect(editor.state.background).toBe('black'); expect(editor.state.fps).toBe(8);
  next = {...next,active_document:'third'}; await editor.refresh();
  expect(editor.state.background).toBe('transparent'); expect(editor.state.fps).toBe(5);
  next = {...next,active_document:'first'}; await editor.refresh();
  expect(editor.state.background).toBe('white'); expect(editor.state.fps).toBe(15);
  expect(editor.state.representation).toBe('grayscale');
  next = {...next,active_document:'second'}; await editor.refresh();
  expect(editor.state.background).toBe('black'); expect(editor.state.fps).toBe(8);
  expect(editor.state.defaults).toEqual({background:'transparent',fps:5});
});
