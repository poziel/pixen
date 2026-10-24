export function advancePlayback(
  frame: number,
  elapsed: number,
  delta: number,
  durations: readonly number[],
  direction: 1 | -1 = 1,
) {
  if (!durations.length) return { frame: 0, elapsed: 0 };
  const safe = durations.map((d) => Math.max(1, d));
  elapsed = (elapsed + Math.max(0, delta)) % safe.reduce((a, b) => a + b, 0);
  frame = Math.min(frame, safe.length - 1);
  while (elapsed >= safe[frame]) {
    elapsed -= safe[frame];
    frame = (frame + direction + safe.length) % safe.length;
  }
  return { frame, elapsed };
}
export class LatestRequest {
  private serial = 0;
  begin() {
    return ++this.serial;
  }
  current(serial: number) {
    return serial === this.serial;
  }
}
