// ADR 0988: the climb through in-place layers (IIFEs, helpers' callbacks) crosses sixteen layers and says why it stopped; the bound is a
// SEVENTEENTH layer, not the sixteenth that ends at a function that is no layer or at module scope. Chrome rc.21 (probed): `resolve` in a memo and `flush` in
// an action step fire at every depth, 15, 16 and 17 included. This is an open program (no closed-boundary assertion): the exports have unseen callers.
import { action, createMemo, createSignal, flush, resolve } from "solid-js";

const [n] = createSignal(1);
const wait = () => n();

// 15 layers under a memo: placed, a violation (SC2004).
export function Memo15() {
  const m = createMemo(() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { void resolve(wait); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); return n(); });
  return <div>{String(m())}</div>;
}

// 16 layers under a memo: placed, a violation (SC2004).
export function Memo16() {
  const m = createMemo(() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { void resolve(wait); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); return n(); });
  return <div>{String(m())}</div>;
}

// 17 layers under a memo: the bound: outer layers unread, an obligation (SC2004).
export function Memo17() {
  const m = createMemo(() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { void resolve(wait); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); return n(); });
  return <div>{String(m())}</div>;
}

// 15 layers in an action step: placed, a violation (SC2006).
export const act15 = action(function* () {
  (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { flush(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })();
  yield 1;
});

// 16 layers in an action step: placed, a violation (SC2006).
export const act16 = action(function* () {
  (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { flush(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })();
  yield 1;
});

// 17 layers in an action step: the bound: an obligation (SC2006).
export const act17 = action(function* () {
  (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { flush(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })();
  yield 1;
});
