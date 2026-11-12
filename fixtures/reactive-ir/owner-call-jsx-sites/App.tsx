import { createEffect, createMemo, createSignal, createTrackedEffect } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, B2 (the re-review of #220): JSX is a creation as a call is. A component tag runs `createComponent`, which dev builds run under `createRoot`, and a tracked
// expression creates a render effect: on an effect apply's stack with no owner set either throws (rc.21 PRIMITIVE_IN_EFFECT_CALLBACK, SC6002), and under a leaf owner
// it is a forbidden creation (PRIMITIVE_IN_FORBIDDEN_SCOPE, SC3001). The owner-call universe listed creating calls only, so a tag a reference ran on an apply's stack
// (`pick = view`, `view` returning `<Inner />`) had no site and the program certified. Every JSX creation site now ends per code like a call does.

function Inner() {
  return <p>inner</p>;
}

// The apply calls a function that returns a tag through a variable: the tag is evaluated on the apply's stack with no owner (Chrome rc.21: SC6002 at the tag).
let pick: (() => unknown) | undefined;
function view() {
  return <Inner />;
}
function ByReference() {
  pick = view;
  createEffect(
    () => 0,
    () => {
      pick!();
    },
  );
  return <p>by reference</p>;
}

// The component the tag renders is declared inside the function a reference enters.
let pickNested: (() => unknown) | undefined;
function outer() {
  function Local() {
    return <p>local</p>;
  }
  return <Local />;
}
function NestedByReference() {
  pickNested = outer;
  createEffect(
    () => 0,
    () => {
      pickNested!();
    },
  );
  return <p>nested</p>;
}

// A handler that is also saved and called from an apply creates the tag there.
let saved: (() => unknown) | undefined;
function SavedHandler() {
  createEffect(
    () => 0,
    () => {
      saved!();
    },
  );
  return (
    <button
      onClick={(saved = () => {
        void (<Inner />);
      })}
    >
      saved
    </button>
  );
}

// A leaf owner's callback reaches the tag the same way (PRIMITIVE_IN_FORBIDDEN_SCOPE, SC3001).
let pickLeaf: (() => unknown) | undefined;
function viewLeaf() {
  return <Inner />;
}
function LeafByReference() {
  pickLeaf = viewLeaf;
  createTrackedEffect(() => {
    pickLeaf!();
  });
  return <p>leaf</p>;
}

// Controls. A component only a tag renders: rendering enters it under the owner its render sets.
function Rendered() {
  const [count] = createSignal(0);
  const memo = createMemo(() => count());
  return (
    <p>
      <Inner />
      {memo()}
    </p>
  );
}

// The direct call: the effect-phase walk places the tag on the apply's stack and files its own violation, which ends the site.
function viewDirect() {
  return <Inner />;
}
function Direct() {
  createEffect(
    () => 0,
    () => {
      viewDirect();
    },
  );
  return <p>direct</p>;
}

function App() {
  return (
    <p>
      <ByReference />
      <NestedByReference />
      <SavedHandler />
      <LeafByReference />
      <Rendered />
      <Direct />
    </p>
  );
}
render(() => <App />, document.body);
