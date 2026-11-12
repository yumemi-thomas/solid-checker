import { createMemo, createRoot, onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, the follow-up to the review of #220 (ADR 0970): the owner emission asks `named_layer_entry_open` whether a layer around an owner-requiring operation may be entered by
// a route the owner graph holds no edge for, and it adopted the owner-call universe's premises of that walk. Besides the component-render premise (fixture
// owner-emission-component-by-reference), two stops of it let a layer through (N1 and N2 of the check of B1 on #220):
//
// - N2, the stop at an owner-creating layer returned before the function's references: a named function expression the root runs can hand itself on, and the host's call of
//   the saved function runs it with no owner.
// - N1, the caller walk took a generator's body for a placed run: a helper called from a generator's body runs where an iterator is stepped. (The emission already answered
//   this shape on main with its generator-step obligation; the cell keeps it so.)
//
// The review of #221 found two more stops that let a layer through: B1 (a function the census takes for a component that is handed to a wrapper call, below) and B2 (a component
// its module exports while the module's namespace is enumerated: `lib.tsx` and `Namespace.tsx`). B3, direct `eval`, has its own fixture (owner-emission-eval): it opens every layer.

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

// B1 (the review of #221). A function the census takes for a component that is handed to a wrapper (`const Wrapped = hoc(() => { .. return <p/> })`) is entered by whoever
// the wrapper is: `hoc` calls it from a timer, with no owner (Chrome rc.21: NO_OWNER_CLEANUP). Only a primitive's position and a standard-library call that runs it inline
// place a function handed to a call.
function hoc(Component: () => unknown) {
  setTimeout(() => {
    Component();
  }, 0);
  return Component as any;
}
const Wrapped = hoc(() => {
  onCleanup(() => {});
  return <p>wrapped</p>;
});

// Controls of B1. A function that returns JSX and is the whole argument of a primitive the dialect gives an owner for (`createMemo`), or of a standard-library call that runs it inline
// (`map`), is placed: its owner is the primitive's, or the component's own, so only that position enters it.
function PrimitiveOwned() {
  const view = createMemo(() => {
    onCleanup(() => {});
    return <p>memo</p>;
  });
  return <p>{view()}</p>;
}
function InlineRun() {
  return (
    <p>
      {[1].map(() => {
        onCleanup(() => {});
        return <b>x</b>;
      })}
    </p>
  );
}

// Control. A component only a tag renders, handed to nothing: rendering enters it under the owner its render sets.
function TaggedOnly() {
  onCleanup(() => {});
  return <p>tagged</p>;
}

function App() {
  return (
    <p>
      <SelfSaving />
      <StepHelper />
      <NamedUnreferenced />
      <PlainCaller />
      <Wrapped />
      <PrimitiveOwned />
      <InlineRun />
      <TaggedOnly />
    </p>
  );
}
render(() => <App />, document.body);
