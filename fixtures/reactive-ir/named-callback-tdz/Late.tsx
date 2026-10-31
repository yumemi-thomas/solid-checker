import { createEffect, createMemo, createSignal } from "solid-js";

// Positive controls, each confirmed by rc.17 in Chrome: the nested function is
// written before the declarator and called after it.
export function MemoNamedCalledAfter() {
  const [v, setV] = createSignal(0);
  const early = () => createMemo(write);
  const write = () => { setV(1); return 1; };
  early();
  return <p>{v()}</p>;
}

export function EffectNamedCalledAfter() {
  const [v] = createSignal(0);
  const early = () => createEffect(() => 1, read);
  const read = () => { v(); };
  early();
  return <p>{v()}</p>;
}

// `count()` runs before the early mention throws: STRICT_READ_UNTRACKED.
export function CountBeforeEarly() {
  const [count] = createSignal(0);
  [1].forEach(() => { count(); r(); });
  const [r] = createSignal(0);
  return <p>{r()}{count()}</p>;
}
