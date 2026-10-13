import { createMemo, untrack } from "solid-js";

// A package-local clearing wrapper around the real `untrack`: the one-line
// spelling most of the ecosystem uses. Its own row is `inline` + `untracked`.
function runUntracked<T>(fn: () => T): T {
  return untrack(fn);
}

// The control wrapper: calls its callback on the same stack exactly as
// `runUntracked` does, and differs only in not clearing the listener. Its own
// row is `inline` + `ambient-at-execution`.
function runNow<T>(fn: () => T): T {
  return fn();
}

// same-stack / untracked. 2.0's `createMemo` computes during the creating
// call, and `handle` runs inside `runUntracked`'s `untrack`, so the memo
// subscribes nothing it reads. The forwarding seam restates the helper's row
// from the helper's own clearing outward; composing the enclosing memo alone
// published `tracked` here.
export function untrackedThroughLocalWrapper(handle: () => number): () => number {
  return createMemo(() => runUntracked(handle));
}

// same-stack / tracked: identical shape, non-clearing helper. The negative
// control -- the memo subscribes what `handle` reads.
export function trackedThroughLocalHelper(handle: () => number): () => number {
  return createMemo(() => runNow(handle));
}

// queued / ambient-at-execution. A listener runs after this returns, but a
// synchronous `target.dispatchEvent(event)` runs it on the dispatcher's
// stack, inside whatever computation made that call: not a fresh stack.
export function listen(target: EventTarget, handler: (event: Event) => void): void {
  target.addEventListener("change", handler);
}

// queued / ambient-at-execution. The bound function is called by whoever
// holds it, and it hands `callback` on from there.
export function bindArgument(
  run: (callback: () => void) => void,
  callback: () => void
): () => void {
  return run.bind(undefined, callback);
}

// queued / untracked, the fresh-stack control: a timer task runs on an empty
// stack, where no caller's listener can be current.
export function later(callback: () => void): void {
  setTimeout(callback, 0);
}

// An unclassifiable wrapper keeps the row open. `schedule` is the caller's own
// function, so nothing here says when -- or whether -- it runs the arrow, and
// no word for `handle` is honest even though the helper inside it clears.
export function throughUnclassified(
  schedule: (run: () => void) => void,
  handle: () => number
): void {
  schedule(() => {
    runUntracked(handle);
  });
}
