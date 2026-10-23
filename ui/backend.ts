import { invoke, isTauri } from "@tauri-apps/api/core";
export interface FrameInfo {
  id: string;
  duration_ms: number;
}
export interface LayerInfo {
  id: string;
  name: string;
  visible: boolean;
  opacity: number;
}
export interface DocumentInfo {
  generation: number;
  revision: number;
  width: number;
  height: number;
  source_width: number;
  source_height: number;
  alternate_view: boolean;
  can_toggle_view: boolean;
  frames: FrameInfo[];
  layers: LayerInfo[];
  path: string | null;
  dirty: boolean;
}
export interface Inspection {
  frame_id: string;
  reference_frame_id: string | null;
  mode: "composite" | "onion_skin" | "frame_comparison";
  layer_id: string | null;
}
export interface Review {
  report: Record<string, any>;
  images: { role: string; png: string }[];
}
export interface Snapshot {
  document: DocumentInfo | null;
  managed: boolean;
  activity: {
    label: string;
    frame_id: string | null;
    layer_id: string | null;
    busy: boolean;
  } | null;
  inspection: Inspection | null;
  review: Review | null;
  review_stale: boolean;
  event_serial: number;
}
export interface RenderRequest {
  generation: number;
  frame_id: string;
  hidden_layers?: string[];
  override_visibility?: boolean;
  annotations?: boolean;
  event_serial?: number;
  layer_id?: string | null;
  reference_frame_id?: string | null;
  mode?: Inspection["mode"];
  representation?: "color" | "silhouette" | "grayscale";
  sheet?: boolean;
  canvas_width?: number;
  canvas_height?: number;
}
export interface Rendered {
  width: number;
  height: number;
  png: string;
}
export const native = isTauri();
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!native) {
    if (command === "snapshot")
      return {
        document: null,
        managed: false,
        activity: null,
        inspection: null,
        review: null,
        event_serial: 0,
      } as T;
    if (command === "desktop_ready") return undefined as T;
    throw new Error(
      "Open the Tauri desktop to use native document operations.",
    );
  }
  return invoke<T>(command, args);
}
