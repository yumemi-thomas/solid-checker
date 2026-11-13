import { createEffect, createMemo, createRoot } from "solid-js";

// The check of B1 (N1, N2): the program of fixtures/reactive-ir/owner-call-layer-stops with each shape at MODULE scope. The oracle's entry hands every export of this
// file to `render` of the real `@solidjs/web`, so B3 (ADR 0967) keeps a row at every site written inside an exported component on every build; a helper, a generator
// and an iterator at module scope have no function around them, so the walks below are all that answered for them. Chrome rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK at
// each creation (the apply steps the iterator or calls the saved function on its own stack, with no owner). The checker used to end every one with
// `EffectCreationRunsOffApply`: the caller walk took a generator's body for a placed run (N1), and the stop at an owner-creating layer returned before it read the
// function's references (N2).

// N1: a helper called from a generator's body.
function helper1() {
  createMemo(() => 1);
}
function* gen1() {
  helper1();
  yield 1;
}
const it1 = gen1();
export function StepHelper() {
  createEffect(
    () => 0,
    () => {
      for (const _ of it1) {
        void _;
      }
    },
  );
  return <p>helper</p>;
}

// N1: an arrow beside the generator.
const run2 = () => {
  createMemo(() => 2);
};
function* gen2() {
  run2();
  yield 1;
}
const it2 = gen2();
export function StepOuterArrow() {
  createEffect(
    () => 0,
    () => {
      void [...it2];
    },
  );
  return <p>arrow</p>;
}

// N1: `yield*` of a plain function.
function helper3() {
  createMemo(() => 3);
  return [1];
}
function* gen3() {
  yield* helper3();
}
const it3 = gen3();
export function StepYieldStar() {
  createEffect(
    () => 0,
    () => {
      for (const _ of it3) {
        void _;
      }
    },
  );
  return <p>yield-star</p>;
}

// N2: a root's callback that saves itself.
let saved: (() => void) | undefined;
createRoot(function f() {
  if (!saved) {
    saved = f;
    return;
  }
  createMemo(() => 4);
});
export function SelfSaving() {
  createEffect(
    () => 0,
    () => {
      saved!();
    },
  );
  return <p>self-saving</p>;
}
