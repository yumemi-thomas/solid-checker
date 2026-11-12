import { createRoot, onCleanup } from "solid-js";

// ADR 0970: the program of fixtures/reactive-ir/owner-emission-layer-stops, every component exported. Chrome rc.21 reports NO_OWNER_CLEANUP at the `onCleanup` of `SelfSaving` (the
// root's callback saved itself and a timer calls it with no owner) and of `StepHelper` (a timer steps a generator whose body calls the helper); the controls are quiet. The owner
// emission used to certify the first (its stop at an owner-creating layer returned before the function's references); the second was an obligation already.

let saved: (() => void) | undefined;
export function SelfSaving() {
  createRoot(function f() {
    if (!saved) {
      saved = f;
      return;
    }
    onCleanup(() => {});
  });
  setTimeout(() => {
    saved!();
  });
  return <p>self-saving</p>;
}

export function StepHelper() {
  function helper() {
    onCleanup(() => {});
  }
  function* gen() {
    helper();
    yield 1;
  }
  const it = gen();
  setTimeout(() => {
    for (const _ of it) {
      void _;
    }
  });
  return <p>helper</p>;
}

// Controls: Chrome is quiet.
export function NamedUnreferenced() {
  createRoot(function f() {
    onCleanup(() => {});
  });
  return <p>named</p>;
}
export function PlainCaller() {
  function helper() {
    onCleanup(() => {});
  }
  function caller() {
    helper();
  }
  caller();
  return <p>plain</p>;
}
