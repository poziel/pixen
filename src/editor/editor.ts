import { reactive, type InjectionKey, inject } from "vue";
import {
  call,
  type Snapshot,
  type Rendered,
  type RenderRequest,
  type Review,
} from "../services/backend";
import { advancePlayback } from "./timeline/playback";
import type { Tool } from './tools/raster';
import { createBrushSettings } from './tools/interaction';

export function createEditor(transport: typeof call = call) {
  const state = reactive({
    tools: {
      temporaryPan: false,
      left: 'pencil' as Tool, right: 'eraser' as Tool,
      primary: '#20212b', secondary: '#ffffff', colorTarget: 'primary' as 'primary'|'secondary',
      picker: 'rectangle' as 'rectangle'|'wheel'|'circle',
      brushes: {left:createBrushSettings(),right:createBrushSettings()},
      tolerance: 0, contiguous: true, selectionMode: 'replace' as 'replace'|'add'|'subtract',
      moveCopy: false, text: 'Text', textSize: 8,
    },
    snapshot: null as Snapshot | null,
    error: "",
    working: false,
    selectionClear: 0,
    frame: 0,
    layer: null as string | null,
    hidden: [] as string[],
    playing: false,
    fps: 5,
    direction: 1 as 1 | -1,
    elapsed: 0,
    isolatedLayer: null as string | null,
    background: "checkerboard",
    mode: "composite" as RenderRequest["mode"],
    reference: null as string | null,
    representation: "color" as RenderRequest["representation"],
    sheet: false,
    follow: true,
    previewSize: null as { width: number; height: number } | null,
    review: null as Review | null,
    showReview: false,
    annotations: false,
    dialog: "" as "" | "new" | "resize" | "settings",
    zoomReset: 0,
    zoomOne: 0,
    zoomRequest: { serial: 0 } as { serial: number; scale?: number; factor?: number },
    canvasZoom: 100,
    canvasView: { zoom: 1, pan: { x: 0, y: 0 }, fit: true },
    canvasWidth: 0,
    canvasHeight: 0,
  });
  const viewKeys = ['frame','layer','hidden','playing','fps','direction','elapsed','isolatedLayer','background','mode','reference','representation','sheet','annotations','canvasZoom','canvasWidth','canvasHeight','canvasView'] as const;
  const captureView = () => JSON.parse(JSON.stringify(Object.fromEntries(viewKeys.map(key => [key, state[key]]))));
  const defaultView = captureView();
  const documentViews = new Map<string, ReturnType<typeof captureView>>();
  let operationQueue: Promise<unknown> = Promise.resolve();
  let lastGeneration = -1,
    lastEvent = -1;
  let refreshError = "";
  let previousFollow = state.follow;
  const images = new Map<string, Promise<Rendered>>();
  let refreshPromise: Promise<void> | undefined;
  function refresh(): Promise<void> {
    if (refreshPromise) return refreshPromise;
    refreshPromise = performRefresh().finally(() => { refreshPromise = undefined; });
    return refreshPromise;
  }
  async function performRefresh() {
    try {
      const next = await transport<Snapshot>("snapshot");
      if (previousFollow !== state.follow) {
        lastEvent = -1;
        previousFollow = state.follow;
      }
      if (succeededAfterError()) state.error = "";
      const oldId = state.snapshot?.active_document;
      const changedDocument = next.active_document !== oldId;
      if (changedDocument) {
        if (oldId) documentViews.set(oldId, captureView());
        Object.assign(state, JSON.parse(JSON.stringify((next.active_document && documentViews.get(next.active_document)) || defaultView)));
        lastGeneration = -1; lastEvent = -1; images.clear();
        state.previewSize = null; state.review = null; state.showReview = false;
      }
      const old = changedDocument ? undefined : state.snapshot?.document;
      if (next.document?.generation !== lastGeneration) {
        images.clear();
        const doc = next.document;
        const selected = changedDocument ? next.document?.frames[state.frame]?.id : old?.frames[state.frame]?.id;
        const index = doc?.frames.findIndex((f) => f.id === selected) ?? -1;
        state.frame = index >= 0 ? index : 0;
        state.elapsed = 0;
        if (!old && doc) state.playing = doc.frames.length > 1;
        if (doc && !doc.layers.some((l) => l.id === state.layer))
          state.layer = doc.layers[0]?.id ?? null;
        if (doc && !doc.layers.some((l) => l.id === state.isolatedLayer))
          state.isolatedLayer = null;
        if (doc && !doc.frames.some((f) => f.id === state.reference))
          state.reference = null;
        state.hidden =
          (!old && !(changedDocument && next.active_document && documentViews.has(next.active_document))) ||
          old?.path !== doc?.path ||
          old?.alternate_view !== doc?.alternate_view
            ? (doc?.layers.filter((l) => !l.visible).map((l) => l.id) ?? [])
            : state.hidden.filter((id) => doc?.layers.some((l) => l.id === id));
        state.sheet = !!doc?.alternate_view && doc.frames.length === 1;
        lastGeneration = doc?.generation ?? -1;
      }
      if (changedDocument && next.active_document && documentViews.has(next.active_document)) {
        Object.assign(state, JSON.parse(JSON.stringify(documentViews.get(next.active_document))));
        const doc = next.document;
        state.frame = Math.min(state.frame, Math.max(0, (doc?.frames.length ?? 1) - 1));
        state.hidden = state.hidden.filter(id => doc?.layers.some(layer => layer.id === id));
        if (!doc?.layers.some(layer => layer.id === state.layer)) state.layer = doc?.layers[0]?.id ?? null;
        if (!doc?.layers.some(layer => layer.id === state.isolatedLayer)) state.isolatedLayer = null;
        if (!doc?.frames.some(frame => frame.id === state.reference)) state.reference = null;
        if ((doc?.frames.length ?? 0) < 2) state.playing = false;
      }
      if (next.documents) for (const id of documentViews.keys()) if (!next.documents.some(document => document.id === id)) documentViews.delete(id);
      state.snapshot = next;
      if (state.follow && next.event_serial !== lastEvent) {
        const doc = next.document;
        const target =
          next.review?.report.frame_id ??
          next.review?.report.frames?.[0]?.frame_id ??
          next.inspection?.frame_id ??
          next.activity?.frame_id;
        const index = doc?.frames.findIndex((f) => f.id === target) ?? -1;
        if (target && (index < 0 || doc?.alternate_view)) {
          const resolved = await transport<string>("follow_target", {
            frameId: target,
          });
          if (resolved !== "found") return;
        }
        const targetLayer =
          next.review?.report.layer_id ??
          next.inspection?.layer_id ??
          next.activity?.layer_id;
        if (targetLayer && !doc?.layers.some((l) => l.id === targetLayer))
          return;
        const targetReference = next.inspection?.reference_frame_id;
        if (
          targetReference &&
          !doc?.frames.some((f) => f.id === targetReference)
        )
          return;
        if (index >= 0) {
          state.frame = index;
          state.playing = false;
          state.sheet = false;
        }
        if (next.inspection) {
          state.mode = next.inspection.mode;
          state.reference = next.inspection.reference_frame_id;
          state.isolatedLayer = next.inspection.layer_id;
        } else if (!next.review) {
          if (next.activity?.layer_id) state.layer = next.activity.layer_id;
          state.mode = "composite";
          state.isolatedLayer = null;
          state.reference = null;
        }
        if (next.review) {
          state.mode = "composite";
          state.reference = null;
          state.isolatedLayer = next.review.report.layer_id ?? null;
          state.representation = next.review.report.representation ?? "color";
          state.annotations = next.review.report.kind === "sprite_analysis";
          state.review = next.review;
          state.showReview = true;
        }
        lastEvent = next.event_serial;
      }
    } catch (e) {
      refreshError = String(e);
      state.error = refreshError;
    }
  }
  function succeededAfterError() {
    const matched = refreshError && state.error === refreshError;
    refreshError = "";
    return matched;
  }
  function following() {
    return !!state.snapshot?.managed && state.follow;
  }
  function action(command: string, args?: Record<string, unknown>) {
    const next = operationQueue.then(() => performAction(command, args));
    operationQueue = next.catch(() => {});
    return next;
  }
  async function performAction(command: string, args?: Record<string, unknown>) {
    const previousIds = state.snapshot?.documents?.map(doc => doc.id) ?? [];
    state.working = true;
    state.error = "";
    try {
      const result = await transport(command, args);
      if (refreshPromise) await refreshPromise;
      await refresh();
      if (
        (command === "create_document" || command === "open_document") &&
        result !== false && !previousIds.includes(state.snapshot?.active_document ?? '')
      ) {
        state.frame = 0;
        state.elapsed = 0;
        state.hidden =
          state.snapshot?.document?.layers
            .filter((l) => !l.visible)
            .map((l) => l.id) ?? [];
        state.reference = null;
        state.isolatedLayer = null;
        state.mode = "composite";
        state.review = null;
        state.showReview = false;
        state.playing = (state.snapshot?.document?.frames.length ?? 0) > 1;
        state.zoomReset++;
      }
      return result;
    } catch (e) {
      state.error = String(e);
    } finally {
      state.working = false;
    }
  }
  function select(frame: number) {
    state.frame = frame;
    state.elapsed = 0;
    state.playing = false;
    state.sheet = false;
  }
  function step(delta: number) {
    const count = state.snapshot?.document?.frames.length ?? 0;
    if (count) select((state.frame + delta + count) % count);
  }
  function play(direction: 1 | -1) {
    state.playing = !(state.playing && state.direction === direction);
    state.direction = direction;
    state.elapsed = 0;
    state.sheet = false;
    state.mode = 'composite';
  }
  function tick(delta: number) {
    if (
      !state.playing ||
      state.sheet ||
      state.mode !== "composite" ||
      following()
    )
      return;
    const doc = state.snapshot?.document;
    if (!doc || doc.frames.length < 2) return;
    const next = advancePlayback(
      state.frame,
      state.elapsed,
      delta,
      doc.frames.map(() => 1000 / state.fps),
      state.direction,
    );
    state.frame = next.frame;
    state.elapsed = next.elapsed;
  }
  function image(request: RenderRequest) {
    const key = JSON.stringify([state.snapshot?.active_document, request]);
    let promise = images.get(key);
    if (!promise) {
      promise = transport<Rendered>("render", { request, documentId: state.snapshot?.active_document });
      images.set(key, promise);
      promise.catch(() => images.delete(key));
      if (images.size > 128) images.delete(images.keys().next().value!);
    }
    return promise;
  }
  function request(): RenderRequest | null {
    const doc = state.snapshot?.document;
    const frame = doc?.frames[state.frame];
    if (!doc || !frame) return null;
    return {
      generation: doc.generation,
      frame_id: frame.id,
      hidden_layers: [...state.hidden],
      override_visibility: true,
      layer_id: state.isolatedLayer,
      annotations: state.annotations && !state.snapshot?.review_stale,
      ...(state.annotations
        ? { event_serial: state.snapshot?.event_serial }
        : {}),
      mode: state.mode,
      reference_frame_id: state.reference,
      representation: state.representation,
      ...(state.previewSize
        ? {
            canvas_width: state.previewSize.width,
            canvas_height: state.previewSize.height,
          }
        : {}),
    };
  }
  async function review(kind: string) {
    const doc = state.snapshot?.document;
    if (!doc) return;
    const result = await action("review_document", {
      kind,
      frameId: doc.frames[state.frame].id,
      layerId: state.isolatedLayer,
    });
    if (result) {
      state.review = result as Review;
      state.showReview = true;
    }
  }
  return {
    state,
    idle: () => operationQueue,
    refresh,
    action,
    readCel: (args: Record<string, unknown>) => transport<{pixels:number[];opacity:number}>('read_cel', args),
    select,
    step,
    play,
    tick,
    image,
    request,
    review,
    following,
  };
}
export type Editor = ReturnType<typeof createEditor>;
export const EditorKey: InjectionKey<Editor> = Symbol("Pixloom");
export function useEditor() {
  const editor = inject(EditorKey);
  if (!editor) throw new Error("Missing Pixloom editor");
  return editor;
}
