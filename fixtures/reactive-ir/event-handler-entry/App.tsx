import { createEffect, createMemo, resolve, until } from "solid-js";

// Proved: an anonymous handler on a DOM element is entered by event delivery alone, off every effect's apply.
export function Resolve() {
  return <button onClick={() => { resolve(() => 1); }}>go</button>;
}

export function Until() {
  return <button onClick={() => { until(() => true); }}>go</button>;
}

// Open: a named handler an effect's apply also calls runs on that apply's stack.
export function Named() {
  const h = () => { resolve(() => 1); };
  createEffect(() => 1, () => { h(); });
  return <button onClick={h}>go</button>;
}

// Misuse twin: resolve in a memo's compute throws (RESOLVE_IN_REACTIVE_SCOPE).
export function InCompute() {
  const m = createMemo(() => { resolve(() => 1); return 1; });
  return <p>{m()}</p>;
}
