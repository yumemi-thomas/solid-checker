// SC2006 flush-in-action on the 2.0.0-rc.8 triple, the first release whose
// @solidjs/signals throws FLUSH_IN_ACTION (`dist/dev-shared.js:1904-1913`;
// `action`'s step raises the depth around `it.next(v)`, `dist/dev.js:1682-1690`).
// The same positives as `rc9-flush-in-action`, and its after-await negative.
import { action, flush } from "solid-js";

// Positive: the head of a sync generator body.
export const syncHead = action(function* (text: string) {
  flush();
  yield text;
});

// Positive: an async generator's first step, with `flush(fn)`.
export const asyncHead = action(async function* (id: number) {
  flush(() => id);
  const saved = await Promise.resolve(id);
  yield saved;
});

// Negative on every release: after an await, outside any step.
export const asyncAfterAwait = action(async function* () {
  await Promise.resolve();
  flush();
});
