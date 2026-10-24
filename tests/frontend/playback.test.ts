import { expect, test } from "bun:test";
import { advancePlayback, LatestRequest } from "../../src/editor/timeline/playback";
test("playback respects durations, wraps, and never modifies saved timing", () => {
  const durations = [100, 200, 50];
  expect(advancePlayback(0, 0, 310, durations)).toEqual({
    frame: 2,
    elapsed: 10,
  });
  expect(advancePlayback(2, 10, 100, durations)).toEqual({
    frame: 0,
    elapsed: 60,
  });
  expect(durations).toEqual([100, 200, 50]);
});
test("only the latest asynchronous selection may paint", () => {
  const pending = new LatestRequest();
  const first = pending.begin();
  const second = pending.begin();
  expect(pending.current(first)).toBe(false);
  expect(pending.current(second)).toBe(true);
});
