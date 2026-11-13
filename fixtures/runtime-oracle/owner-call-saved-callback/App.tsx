import { createEffect, createMemo, untrack } from "solid-js";

// The review of #220 (F2): a handler, a timer callback or a generator body that is also entered by a route the role engine does not follow (saved to a variable or an
// array, stepped from an apply). The program of fixtures/reactive-ir/owner-call-saved-callback. Chrome rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK at each creation but the
// two controls (`Timer`, `Body`); the checker used to end every one with `EffectCreationRunsOffApply` (the lexical role alone).

let saved: (() => void) | undefined;
export function Saved() {
  createEffect(
    () => 0,
    () => {
      saved!();
    },
  );
  return <button onClick={(saved = () => { createMemo(() => 1); })}>saved</button>;
}

let savedTimer: (() => void) | undefined;
export function SavedTimer() {
  createEffect(
    () => 0,
    () => {
      savedTimer!();
    },
  );
  setTimeout((savedTimer = () => { createMemo(() => 2); }), 1000);
  return <p>timer</p>;
}

export function InArray() {
  const handler = () => {
    createMemo(() => 3);
  };
  const all = [handler];
  createEffect(
    () => 0,
    () => {
      all[0]!();
    },
  );
  return <button onClick={handler}>array</button>;
}

export function StepGenerator() {
  function* steps() {
    createMemo(() => 4);
    yield 1;
  }
  const it = steps();
  createEffect(
    () => 0,
    () => {
      it.next();
    },
  );
  return <p>generator</p>;
}

export function StepUntrack() {
  const it = untrack(function* () {
    createMemo(() => 5);
    yield 1;
  });
  createEffect(
    () => 0,
    () => {
      it.next();
    },
  );
  return <p>untrack</p>;
}

// Controls: Chrome is quiet at both creations.
export function Timer() {
  setTimeout(() => {
    createMemo(() => 6);
  });
  return <p>timer</p>;
}
export function Body() {
  const memo = createMemo(() => 7);
  return <p>{memo()}</p>;
}
