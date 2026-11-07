import { createMemo, createSignal, flush } from "solid-js";

// `flush(fn)` keeps the caller's observer, for a read in an IIFE `fn` runs too:
// in a helper, the helper's caller decides. Tracked JSX is silent in Chrome.
export function InJsxEach() {
  const [s] = createSignal(1);
  const k = () => { flush(() => { (() => { s(); })(); }); return 1; };
  return <p>{(() => { [1].forEach(k); return 1; })()}</p>;
}

export function InJsxCall() {
  const [s] = createSignal(1);
  const k = () => { flush(() => { (() => { s(); })(); }); return 1; };
  return <p>{(k(), 1)}</p>;
}

// The same read directly in the callback answers the same.
export function InJsxDirect() {
  const [s] = createSignal(1);
  const k = () => { flush(() => { s(); }); return 1; };
  return <p>{(() => { [1].forEach(k); return 1; })()}</p>;
}

// Chrome warns, but a helper's call is no proof the IIFE runs in the body: open.
export function HelperInBody() {
  const [s] = createSignal(1);
  const k = () => { flush(() => { (() => { s(); })(); }); return 1; };
  k();
  return <p>{1}</p>;
}

// Written in the body, through the flush: Chrome warns.
export function InBody() {
  const [s] = createSignal(1);
  flush(() => { (() => { s(); })(); });
  return <p>{1}</p>;
}

// In a memo's flush: tracked.
export function InMemo() {
  const [s] = createSignal(1);
  const m = createMemo(() => { flush(() => { (() => { s(); })(); }); return 1; });
  return <p>{m()}</p>;
}

// Review R15: a helper that runs the literal and keeps it: the kept run is in
// the body, so the tracked in-place run clears nothing: open.
export function RunAndKept() {
  const [s] = createSignal(1);
  const saved: (() => void)[] = [];
  const twice = (cb: () => void) => { cb(); saved.push(cb); };
  const m = createMemo(() => { flush(() => { twice(() => { s(); }); }); return 1; });
  saved[0]?.();
  return <p>{m()}</p>;
}
