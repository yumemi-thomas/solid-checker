// The tracer for ADR 0159: every export calls its callback directly in its
// own body, so ADR 0100's census confirms a `callbacks` item for it. Only the
// first runs it on every normal completion; a claim that the callback runs at
// least once (`min >= 1`) is true of it alone.

export function always(callback) {
  callback();
}

// A `throw` before the call leaves every normal completion running it.
export function afterThrow(callback, message) {
  if (message === undefined) throw "missing message";
  callback();
}

// Each of these runs the callback on some invocations only.

export function guarded(callback, flag) {
  if (flag) callback();
}

export function shortCircuit(callback, flag) {
  flag && callback();
}

export function chosen(callback, flag) {
  return flag ? callback() : undefined;
}

export function optional(callback) {
  callback?.();
}

export function early(callback, flag) {
  if (flag) return;
  callback();
}

export function looped(callback, count) {
  for (let i = 0; i < count; i++) callback();
}
