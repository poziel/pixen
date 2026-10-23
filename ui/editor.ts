import { reactive, type InjectionKey, inject } from "vue";
import {
  call,
  type Snapshot,
  type Rendered,
  type RenderRequest,
  type Review,
} from "./backend";
import { advancePlayback } from "./playback";

export function createEditor(transport: typeof call = call) {
  const state = reactive({
    snapshot: null as Snapshot | null,
    error: "",
    working: false,
    frame: 0,
    layer: null as string | null,
    hidden: [] as string[],
    playing: false,
    speed: 1,
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
  });
  let polling = false,
    lastGeneration = -1,
    lastEvent = -1;
  let refreshError = "";
  let previousFollow = state.follow;
  const images = new Map<string, Promise<Rendered>>();
  async function refresh() {
    if (polling) return;
    polling = true;
    try {
      const next = await transport<Snapshot>("snapshot");
      if (previousFollow !== state.follow) {
        lastEvent = -1;
        previousFollow = state.follow;
      }
      if (succeededAfterError()) state.error = "";
      const old = state.snapshot?.document;
      if (next.document?.generation !== lastGeneration) {
        images.clear();
        const doc = next.document;
        const selected = old?.frames[state.frame]?.id;
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
          !old ||
          old.path !== doc?.path ||
          old.alternate_view !== doc?.alternate_view
            ? (doc?.layers.filter((l) => !l.visible).map((l) => l.id) ?? [])
            : state.hidden.filter((id) => doc?.layers.some((l) => l.id === id));
        state.sheet = !!doc?.alternate_view && doc.frames.length === 1;
        lastGeneration = doc?.generation ?? -1;
      }
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
    } finally {
      polling = false;
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
  async function action(command: string, args?: Record<string, unknown>) {
    if (state.working) return;
    state.working = true;
    state.error = "";
    try {
      const result = await transport(command, args);
      await refresh();
      if (
        (command === "create_document" || command === "open_document") &&
        result !== false
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
      delta * state.speed,
      doc.frames.map((f) => f.duration_ms),
    );
    state.frame = next.frame;
    state.elapsed = next.elapsed;
  }
  function image(request: RenderRequest) {
    const key = JSON.stringify(request);
    let promise = images.get(key);
    if (!promise) {
      promise = transport<Rendered>("render", { request });
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
    refresh,
    action,
    select,
    step,
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
