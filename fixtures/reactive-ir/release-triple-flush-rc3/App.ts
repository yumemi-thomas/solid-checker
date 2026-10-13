// flush() in action bodies on the audited 2.0.0-rc.3 triple, whose
// @solidjs/signals has no action-step guard: `flush` drains inside a step as
// anywhere else, in dev and production (the rc.1-rc.8 release review § 3,
// probe R: `resolved` on rc.0-rc.7). SC2006 states rc.8's throw, which these
// bytes cannot make true, so nothing here is reported. The same source on rc.8
// is `release-triple-flush-rc8`.
import { action, flush } from "solid-js";

// Silent on rc.3 (reported on rc.8): the head of a sync generator body.
export const syncHead = action(function* (text: string) {
  flush();
  yield text;
});

// Silent on rc.3 (reported on rc.8): an async generator's first step, with `flush(fn)`.
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
