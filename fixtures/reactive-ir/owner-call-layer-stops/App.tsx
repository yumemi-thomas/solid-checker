import { createEffect, createMemo, createRoot } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, slice 6, the check of B1 (N1, N2): the owner-call universe's silences speak for a site only where every layer around it is entered the way its role says.
// Two walks stopped too early. The caller walk of the entry test (`layer_entry_open`) followed a call out of a helper to a generator's body and took that body for a
// placed run (N1: a generator's body runs where an iterator is stepped), and its stop at an owner-creating layer returned before it read the function's references (N2: a
// named function expression can hand itself on).

// N1. A helper written outside the generator, called from its body; the apply steps the generator with a `for...of`: the helper runs on the apply's stack with no owner.
function StepHelper() {
  function helper() {
    createMemo(() => 1);
  }
  function* gen() {
    helper();
    yield 1;
  }
  const it = gen();
  createEffect(
    () => 0,
    () => {
      for (const _ of it) {
        void _;
      }
    },
  );
  return <p>helper</p>;
}

// N1. An arrow written beside the generator.
function StepOuterArrow() {
  const run = () => {
    createMemo(() => 2);
  };
  function* gen() {
    run();
    yield 1;
  }
  const it = gen();
  createEffect(
    () => 0,
    () => {
      void [...it];
    },
  );
  return <p>arrow</p>;
}

// N1. `yield*` of a plain function that creates and returns an array.
function StepYieldStar() {
  function helper() {
    createMemo(() => 3);
    return [1];
  }
  function* gen() {
    yield* helper();
  }
  const it = gen();
  createEffect(
    () => 0,
    () => {
      for (const _ of it) {
        void _;
      }
    },
  );
  return <p>yield-star</p>;
}

// N2. The root's callback saves itself on its first run (the root runs it at once and nothing is created); the apply calls it later, on its own stack, with no owner.
let saved: (() => void) | undefined;
function SelfSaving() {
  createRoot(function f() {
    if (!saved) {
      saved = f;
      return;
    }
    createMemo(() => 4);
  });
  createEffect(
    () => 0,
    () => {
      saved!();
    },
  );
  return <p>self-saving</p>;
}

// Controls. A root's named callback nothing references: the root's owner is current there, so the creation is silent.
function NamedUnreferenced() {
  createRoot(function f() {
    createMemo(() => 5);
  });
  return <p>named</p>;
}

// A helper called only from a plain function the body calls: rendering enters it, no generator around.
function PlainCaller() {
  function helper() {
    createMemo(() => 6);
  }
  function caller() {
    helper();
  }
  caller();
  return <p>plain</p>;
}

function App() {
  return (
    <p>
      <StepHelper />
      <StepOuterArrow />
      <StepYieldStar />
      <SelfSaving />
      <NamedUnreferenced />
      <PlainCaller />
    </p>
  );
}
render(() => <App />, document.body);
