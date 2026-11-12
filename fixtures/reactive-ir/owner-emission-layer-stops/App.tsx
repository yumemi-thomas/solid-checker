import { createRoot, onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, the follow-up to the review of #220 (ADR 0970): the owner emission asks `named_layer_entry_open` whether a layer around an owner-requiring operation may be entered by
// a route the owner graph holds no edge for, and it adopted the owner-call universe's premises of that walk. Besides the component-render premise (fixture
// owner-emission-component-by-reference), two stops of it let a layer through (N1 and N2 of the check of B1 on #220):
//
// - N2, the stop at an owner-creating layer returned before the function's references: a named function expression the root runs can hand itself on, and the host's call of
//   the saved function runs it with no owner.
// - N1, the caller walk took a generator's body for a placed run: a helper called from a generator's body runs where an iterator is stepped. (The emission already answered
//   this shape on main with its generator-step obligation; the cell keeps it so.)

// N2. The root runs the callback at once (it saves itself and creates nothing); the timer calls it later, on its own stack: no owner is current (Chrome rc.21: NO_OWNER_CLEANUP).
let saved: (() => void) | undefined;
function SelfSaving() {
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

// N1. A helper beside a generator, called from its body; a timer steps the iterator: no owner is current.
function StepHelper() {
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

// Controls. A root's named callback nothing references: the root's owner is current there, so the cleanup is owned.
function NamedUnreferenced() {
  createRoot(function f() {
    onCleanup(() => {});
  });
  return <p>named</p>;
}

// A helper called only from a plain function the body calls: rendering enters it, no generator around.
function PlainCaller() {
  function helper() {
    onCleanup(() => {});
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
      <SelfSaving />
      <StepHelper />
      <NamedUnreferenced />
      <PlainCaller />
    </p>
  );
}
render(() => <App />, document.body);
