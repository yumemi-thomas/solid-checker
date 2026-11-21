import { createSignal, createTrackedEffect } from "solid-js";
import { hand, handMemo, handTwice } from "r1155-hand";

// ADR 1155 (a), the Opus review of ADR 1147 (F1's root): a write in a literal a function forwards, by ADR 0200's exact proof, to a leaf owner's slot
// (createTrackedEffect's compute) runs only in that leaf callback, which rc.21's write guard exempts as it does the leaf's own literal. Chrome: quiet.

// Positive: a package read as source forwards it to createTrackedEffect (the review's s11).
export function HandApp() {
  const [n, setN] = createSignal(0);
  hand(() => setN(1));
  return <i>{n()}</i>;
}

// Positive: the same forwarding written in the project.
function ownHand(fn: () => void) {
  createTrackedEffect(() => {
    fn();
  });
}
export function OwnHandApp() {
  const [n, setN] = createSignal(0);
  ownHand(() => setN(2));
  return <i>{n()}</i>;
}

// Negative (the misuse twin): the package forwards it to a memo's compute; rc.21 throws REACTIVE_WRITE_IN_OWNED_SCOPE, the violation stays.
export function MemoApp() {
  const [n, setN] = createSignal(0);
  handMemo(() => setN(3));
  return <i>{n()}</i>;
}

// Negative: the package also hands the parameter to a timer: no exact forwarding, the write keeps the row it had.
export function TwiceApp() {
  const [n, setN] = createSignal(0);
  handTwice(() => setN(4));
  return <i>{n()}</i>;
}
