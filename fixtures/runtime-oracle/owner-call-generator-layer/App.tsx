import { createEffect, createMemo, createRoot, untrack } from "solid-js";

// The second re-review of #220 (B1): a creation written in a layer inside a generator (an arrow, an IIFE or an `untrack` callback the generator's body runs in place),
// stepped from an effect's apply with a `for...of` or a spread. The program of fixtures/reactive-ir/owner-call-generator-layer. Chrome rc.21 throws
// PRIMITIVE_IN_EFFECT_CALLBACK at the first three creations and at `createRoot` in StepRoot (the memo in its callback never runs); Idle is quiet. The checker used to end
// the first three with `EffectCreationRunsOffApply` (the test that a site is plain asked the innermost function only), but only in a program no unread package may run: this
// harness hands every export to `render` of the real `@solidjs/web`, so B3 keeps a row at each creation on every build. The false clean is the closed-boundary run of the
// fixture (its snapshot) and the fuzz cells er-genlayer-apply and er-genuntrack-apply (module-scope generators), which Chrome confirms.

export function StepForOf() {
  function* steps() {
    const run = () => {
      createMemo(() => 1);
    };
    run();
    yield 1;
  }
  const it = steps();
  createEffect(
    () => 0,
    () => {
      for (const _ of it) {
        void _;
      }
    },
  );
  return <p>for-of</p>;
}

export function StepSpread() {
  function* steps() {
    untrack(() => {
      createMemo(() => 2);
    });
    yield 1;
  }
  const it = steps();
  createEffect(
    () => 0,
    () => {
      void [...it];
    },
  );
  return <p>spread</p>;
}

export function StepIife() {
  function* steps() {
    (() => {
      createMemo(() => 3);
    })();
    yield 1;
  }
  const it = steps();
  createEffect(
    () => 0,
    () => {
      for (const _ of it) {
        void _;
      }
    },
  );
  return <p>iife</p>;
}

export function StepRoot() {
  function* steps() {
    createRoot(() => {
      createMemo(() => 4);
    });
    yield 1;
  }
  const it = steps();
  createEffect(
    () => 0,
    () => {
      for (const _ of it) {
        void _;
      }
    },
  );
  return <p>root</p>;
}

// Control: Chrome is quiet (the iterator is never stepped).
export function Idle() {
  function* steps() {
    const run = () => {
      createMemo(() => 5);
    };
    run();
    yield 1;
  }
  const it = steps();
  createEffect(
    () => 0,
    () => {
      void it;
    },
  );
  return <p>idle</p>;
}
