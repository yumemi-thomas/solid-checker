import { createSignal } from "solid-js";

// `make` runs its parameter inline and returns a closure that never runs its
// own parameter. `ok` runs its parameter inline and returns nothing.
export function make(cb: () => void) {
  cb();
  return (_other: () => void) => {};
}
export function ok(cb: () => void) {
  cb();
}

// Negative: the callee `make(() => {})` is a call. Its value is the returned
// closure, which never runs `() => s(1)`. The entity at the callee's span names
// `make` (the inner call), so the outer call is not `make`'s inline argument.
export function CurriedWrite() {
  const [c, s] = createSignal(0);
  make(() => {})(() => s(1));
  return <p>{c()}</p>;
}

// Negative, the read form of the same shape.
export function CurriedRead() {
  const [c] = createSignal(0);
  make(() => {})(() => c());
  return <p>hi</p>;
}

// Negative control: `g` is a call result, and no fact models what `make`
// returns, so `g(() => s(1))` is unknown, never a proven inline write.
export function CallResultControl() {
  const [c, s] = createSignal(0);
  const g = make(() => {});
  g(() => s(1));
  return <p>{c()}</p>;
}

// Positive controls: an identifier callee that runs its argument inline is a
// violation, and so is a direct argument to `make`.
export function InlineWrite() {
  const [c, s] = createSignal(0);
  ok(() => s(1));
  return <p>{c()}</p>;
}
export function DirectWrite() {
  const [c, s] = createSignal(0);
  make(() => s(1));
  return <p>{c()}</p>;
}
