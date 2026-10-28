import {
  createEffect,
  createMemo,
  createRenderEffect,
  createRoot,
  createSignal,
  getOwner,
  runWithOwner,
  untrack,
} from "solid-js";

// ADR 0483: creating a computation or root with no owner in a createEffect
// apply throws PRIMITIVE_IN_EFFECT_CALLBACK on rc.17 dev.

// Violations.
export function MemoInApply() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    (value) => {
      createMemo(() => value * 2);
    },
  );
  return null;
}

export function RootInApply() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      createRoot(() => 1);
    },
  );
  return null;
}

export function DerivedSignalThroughIife() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    (value) => {
      (() => createSignal(() => value))();
    },
  );
  return null;
}

export function MemoUnderUntrack() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      untrack(() => createMemo(() => 1));
    },
  );
  return null;
}

// Uncertifiable: an earlier call in the callback may run an effect
// synchronously, which lowers the marker the guard reads.
export function AfterAnotherCall() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      console.log("first");
      createMemo(() => 1);
    },
  );
  return null;
}

// Clean: an owner is set, the state is value-form, or the callback is the
// compute (owned) or a render effect's apply (first run owned).
export function OwnedAndValueForm() {
  const [count] = createSignal(0);
  const owner = getOwner();
  createEffect(
    () => count(),
    () => {
      runWithOwner(owner, () => createMemo(() => 1));
    },
  );
  createEffect(
    () => count(),
    () => {
      createSignal(0);
    },
  );
  createEffect(
    () => createMemo(() => count()),
    () => {},
  );
  createRenderEffect(
    () => count(),
    () => {
      createMemo(() => 1);
    },
  );
  return null;
}
