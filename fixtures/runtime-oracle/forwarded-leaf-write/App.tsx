import { createMemo, createSignal, createTrackedEffect } from "solid-js";

// ADR 1155 (a), the Opus review of ADR 1147 (F1's root): a write in a literal a function forwards, by ADR 0200's exact proof, to a leaf owner's slot
// (createTrackedEffect's compute) runs only in that leaf callback, which rc.21's write guard exempts as it does the leaf's own literal. The review's probe
// forwards through a package read as source (patterns `ctxmemo/s11-hand-direct`, the generator's `barm-fwdleaf-*`); the same proof reads a project function.

function hand(fn: () => void) {
  createTrackedEffect(() => {
    fn();
  });
}

function handMemo(fn: () => void) {
  createMemo(() => {
    fn();
    return 0;
  });
}

function handTwice(fn: () => void) {
  createTrackedEffect(() => {
    fn();
  });
  setTimeout(fn, 0);
}

// Positive: forwarded to createTrackedEffect's compute (main: SC2001 a violation; Chrome quiet).
export function HandApp() {
  const [n, setN] = createSignal(0);
  hand(() => setN(1));
  return <i>{n()}</i>;
}

// Negative (the misuse twin): forwarded to a memo's compute; rc.21 throws REACTIVE_WRITE_IN_OWNED_SCOPE, the violation stays.
export function MemoApp() {
  const [n, setN] = createSignal(0);
  handMemo(() => setN(2));
  return <i>{n()}</i>;
}

// Negative: the parameter is also handed to a timer: no exact forwarding, the write keeps the row it had.
export function TwiceApp() {
  const [n, setN] = createSignal(0);
  handTwice(() => setN(3));
  return <i>{n()}</i>;
}

// Negative (the review of 1155, F1, Chrome: REACTIVE_WRITE_IN_OWNED_SCOPE): a non-arrow wrapper also calls its argument through `arguments`, here in the memo
// that calls it. The parameter's references are not the argument's only reads: no exact forwarding, the violation stays.
function handArgs(fn: () => void) {
  createTrackedEffect(() => {
    fn();
  });
  arguments[0]();
}
export function ArgsApp() {
  const [n, setN] = createSignal(0);
  createMemo(() => {
    handArgs(() => setN(4));
    return 0;
  });
  return <i>{n()}</i>;
}

// Negative (F1): the parameter reassigned in the wrapper: the slot may run another function, the literal's own run is not placed: the row it had stays.
function handSwap(fn: () => void, other: () => void) {
  fn = other;
  createTrackedEffect(() => {
    fn();
  });
}
export function SwapApp() {
  const [n, setN] = createSignal(0);
  createMemo(() => {
    handSwap(
      () => setN(5),
      () => {},
    );
    return 0;
  });
  return <i>{n()}</i>;
}

// Positive (ADR 1155 r2): a non-arrow wrapper that reads only `arguments.length` runs no argument through it: the forwarding to the leaf stands, clean.
function handLength(fn: () => void) {
  createTrackedEffect(() => {
    fn();
  });
  void arguments.length;
}
export function LengthApp() {
  const [n, setN] = createSignal(0);
  handLength(() => setN(6));
  return <i>{n()}</i>;
}
