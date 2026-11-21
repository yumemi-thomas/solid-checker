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
