import { createMemo } from "solid-js";

// The tracer for ADR 0162's generator half: an export whose every
// value-carrying completion is, whole, a `createMemo` call proposes a described
// callable that reads a memo it created and hands back what it read.

// A direct memo over a literal compute.
export function createDoubled(count) {
  return createMemo(() => count() * 2);
}

// The same, as an expression-bodied arrow (the shape `signal-builders` uses).
export const createLabel = source => createMemo(() => "n=" + source());

// The computation is the caller's own callable. The read may run it, and that
// run is the registration's, not the read's, so this proposes the same read.
export function createLive(compute) {
  return createMemo(compute);
}

// Several completions, every one a whole `createMemo` call.
export function createGuarded(flag, a, b) {
  if (flag) return createMemo(a);
  return createMemo(b);
}

// A conditional of memos: the producer states no literal for either arm, so
// nothing is proposed for the described shape and ADR 0113's plain proposal
// stands (the census refuses it by name).
export function createEither(flag, a, b) {
  return flag ? createMemo(a) : createMemo(b);
}

// A binding that holds the memo and is then written to: not a whole `createMemo`
// completion, so nothing is proposed.
export function createBound(source) {
  const memo = createMemo(() => source());
  memo.extra = 1;
  return memo;
}

// The memo's own read is called: the completion is a value, not the accessor.
export function readOnce(source) {
  return createMemo(() => source())();
}

// A local function spelled `createMemo`. The spelling proposes; the census
// decides by resolved identity and withdraws the return by name.
export function createShadowed(fn) {
  const createMemo = f => f;
  return createMemo(fn);
}
