import { createEffect, createMemo, createTrackedEffect, onCleanup } from "solid-js";

// The review of #220 (F1): a function that returns JSX, entered by a reference no tag is, and called from an effect's apply or a leaf owner's callback (the program of
// fixtures/reactive-ir/owner-call-component-by-reference). Chrome rc.21 throws at each creation and at the `onCleanup`; the checker used to end the sites with the owner
// graph's silences (`OwnerCurrent`, `LeafOwnerAbsent`), which trusted the component-render premise. Now each keeps its uncertifiable row.

function Tagged() {
  const memo = createMemo(() => 1);
  return <p>{memo()}</p>;
}
// The control: a component only a tag renders; Chrome is quiet.
export function TaggedOnly() {
  return (
    <p>
      <Tagged />
    </p>
  );
}

let pick: (() => unknown) | undefined;
function view() {
  const memo = createMemo(() => 2);
  return <p>{memo()}</p>;
}
export function Picks() {
  pick = view;
  createEffect(
    () => 0,
    () => {
      pick!();
    },
  );
  return <p>picks</p>;
}

let pickRendered: (() => unknown) | undefined;
function Rendered() {
  const memo = createMemo(() => 3);
  return <p>{memo()}</p>;
}
export function PicksRendered() {
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

let pickLeaf: (() => unknown) | undefined;
function Leaf() {
  const memo = createMemo(() => 4);
  onCleanup(() => {});
  return <p>{memo()}</p>;
}
export function PicksLeaf() {
  pickLeaf = Leaf;
  createTrackedEffect(() => {
    pickLeaf!();
  });
  return <p>leaf</p>;
}
