import { createEffect, createMemo, createSignal } from "solid-js";

// A callback named in a nested function that runs before the callback's
// declarator: evaluating the argument throws "Cannot access 'write' before
// initialization" (rc.17 in Chrome, no Solid diagnostic), and `tsc` is silent
// in a nested function. Uncertifiable, never a violation (ADR 0517).
export function MemoNamedEarly() {
  const [v, setV] = createSignal(0);
  const early = () => createMemo(write);
  early();
  const write = () => { setV(1); return 1; };
  return <p>{v()}</p>;
}

export function EffectNamedEarly() {
  const [v] = createSignal(0);
  const early = () => createEffect(() => 1, read);
  early();
  const read = () => { v(); };
  return <p>{v()}</p>;
}

// `r()` throws before `count()` runs (an ADR 0464 residual).
export function CountAfterEarly() {
  const [count] = createSignal(0);
  [1].forEach(() => { r(); count(); });
  const [r] = createSignal(0);
  return <p>{r()}{count()}</p>;
}
