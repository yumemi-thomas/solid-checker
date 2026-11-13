import { createEffect, createMemo, createRoot, untrack } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, the second re-review of #220 (B1): a generator's body runs where an iterator is stepped, and the role engine, which places a generator's body by the call that
// makes the iterator, certified a creation written in a layer INSIDE a generator (an arrow, an IIFE or an `untrack` callback its body runs in place): the test of the
// innermost function saw a plain arrow. The question is asked of every layer up to the one whose callback runs under an owner its primitive creates, which is current at
// the site whoever steps what is around it.

// An arrow the generator's body calls holds the creation; the apply steps the generator with a `for...of` (no call the checker cannot follow stands at the stepper).
function StepForOf() {
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

// An `untrack` callback the generator's body runs in place; the apply spreads the iterator.
function StepSpread() {
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

// An IIFE in the generator's body.
function StepIife() {
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

// The owner layer stops the climb: the memo is in `createRoot`'s callback, whose owner is current there however the generator is stepped, so the creation is silent
// (`effect-creation-owner-set-around`). The `createRoot` call is the generator's own body, a site no entry places: it keeps its uncertifiable row (and it is what throws).
function StepRoot() {
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

// The cost: the iterator is made and never stepped, so the creation never runs (Chrome is quiet), but the generator layer is no plain site and the creation keeps its row.
function Idle() {
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

function App() {
  return (
    <p>
      <StepForOf />
      <StepSpread />
      <StepIife />
      <StepRoot />
      <Idle />
    </p>
  );
}
render(() => <App />, document.body);
