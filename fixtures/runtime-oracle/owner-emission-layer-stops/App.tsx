import { createRoot, onCleanup } from "solid-js";

// ADR 0970: the program of fixtures/reactive-ir/owner-emission-layer-stops, every component exported. Chrome rc.21 reports NO_OWNER_CLEANUP at the `onCleanup` of `SelfSaving` (the
// root's callback saved itself and a timer calls it with no owner), of `StepHelper` (a timer steps a generator whose body calls the helper) and of `Wrapped` (the wrapper `hoc`
// calls its argument from a timer, B1 of the review of #221); the controls are quiet. The owner emission used to certify the first and the third (its stop at an owner-creating
// layer returned before the function's references; a function handed to a wrapper call was a component the census proved); the second was an obligation already.

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

// B1 (the review of #221). The wrapper calls what it is handed from a timer, with no owner: the census takes the arrow for a component (`hoc_bound`), the wrapper's call of it is
// not a render.
function hoc(Component: () => unknown) {
  setTimeout(() => {
    Component();
  }, 0);
  return Component as any;
}
export const Wrapped = hoc(() => {
  onCleanup(() => {});
  return <p>wrapped</p>;
});
