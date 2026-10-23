import { expect, test } from "bun:test";
import { createEditor } from "./editor";
import type { Snapshot } from "./backend";

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
