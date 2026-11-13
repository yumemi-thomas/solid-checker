import { createEffect, createMemo, createTrackedEffect, onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, slice 6, the review of #220 (F1): the graph gives a function that returns JSX an owned context, and the emission's test of what enters it
// (`named_layer_entry_open`, ADR 0868) stopped at such a function without looking at its references: the component-render premise the ruling excludes. The owner-call
// universe's silences (`OwnerCurrent` for SC6002, `LeafOwnerAbsent` for SC3001) now ask the test without the premise: a function that returns JSX is entered by renders
// alone only where every reference of its binding is a JSX tag; one a reference enters is a layer like any other, and its creation keeps the uncertifiable row.

// A component only a tag renders: its entries are renders, so an owner is current at the creation (`OwnerCurrent`; the role engine places rendering off every apply).
function Tagged() {
  const memo = createMemo(() => 1);
  return <p>{memo()}</p>;
}

// The same shape, a function that returns JSX, with no tag at all: a component the effect's apply picks up and calls directly. The apply's stack has no owner, so
// the creation meets `inEffectCallback && context === null` (PRIMITIVE_IN_EFFECT_CALLBACK, Chrome rc.21). The row is `effect-creation-unaccounted`, never silence.
let pick: (() => unknown) | undefined;
function view() {
  const memo = createMemo(() => 2);
  return <p>{memo()}</p>;
}
function Picks() {
  pick = view;
  createEffect(
    () => 0,
    () => {
      pick!();
    },
  );
  return <p>picks</p>;
}

// A component a tag renders and a reference also enters (an escaped component): the reference is an entry no edge shows, so the render does not account for every run.
let pickRendered: (() => unknown) | undefined;
function Rendered() {
  const memo = createMemo(() => 3);
  return <p>{memo()}</p>;
}
function PicksRendered() {
  pickRendered = Rendered;
  createEffect(
    () => 0,
    () => {
      pickRendered!();
    },
  );
  return (
    <p>
      <Rendered />
    </p>
  );
}

// The same through a leaf owner: `createTrackedEffect`'s callback runs the function a reference picked, where a creation and an `onCleanup` throw
// (PRIMITIVE_IN_FORBIDDEN_SCOPE, CLEANUP_IN_FORBIDDEN_SCOPE). The program names a leaf opener, so the census does not answer: each of the two keeps its row
// (`leaf-operation-unaccounted`), and so does the creation's SC6002 code (the leaf callback is no apply, but nothing proves that here).
let pickLeaf: (() => unknown) | undefined;
function Leaf() {
  const memo = createMemo(() => 4);
  onCleanup(() => {});
  return <p>{memo()}</p>;
}
function PicksLeaf() {
  pickLeaf = Leaf;
  createTrackedEffect(() => {
    pickLeaf!();
  });
  return <p>leaf</p>;
}

// The root: every component above is rendered from here, so `Tagged` has an owner edge.
function App() {
  return (
    <p>
      <Tagged />
      <Picks />
      <PicksRendered />
      <PicksLeaf />
    </p>
  );
}
render(() => <App />, document.body);
