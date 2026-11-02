import { action, createMemo, createRoot, createSignal, createTrackedEffect, onCleanup, onSettled } from "solid-js";
import { imported } from "./helpers";

// An action's first step runs on the caller's stack under the caller's leaf
// owner: a memo or a cleanup there throws as it does in the leaf itself.
export function FromLeaf() {
  const [r1] = createSignal(0);
  const act = action(function* () {
    createMemo(() => 1);
    onCleanup(() => {});
  });
  onSettled(() => {
    act();
  });
  createTrackedEffect(() => {
    r1();
    act();
  });
  return <div />;
}

// Writes and reads are legal in a leaf; past a yielded promise the step runs
// with no owner, where a root is legal.
export function LegalSteps() {
  const [r1, setR1] = createSignal(0);
  const safe = action(function* () {
    setR1(1);
    r1();
  });
  const later = action(function* () {
    yield Promise.resolve();
    createRoot(() => 1);
  });
  onSettled(() => {
    safe();
    later();
  });
  return <div />;
}

// After a plain `yield`, on one branch, or through an action no `const` here
// holds: possible, an obligation.
export function Unproven() {
  const plain = action(function* () {
    yield 1;
    createMemo(() => 1);
  });
  const branch = action(function* (on: boolean) {
    if (on) createMemo(() => 1);
  });
  onSettled(() => {
    plain();
  });
  onSettled(() => {
    branch(true);
  });
  onSettled(() => {
    imported();
  });
  return <div />;
}
