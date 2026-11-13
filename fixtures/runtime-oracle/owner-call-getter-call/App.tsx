import { createEffect, createMemo, createTrackedEffect } from "solid-js";

// B2 (the review of #225, F2, found while fixing it): a creating CALL written as a component element's prop value is a getter like JSX, run where the component reads the prop. Read
// on an effect's apply it creates a memo on a stack with no owner: Chrome rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK at the call (and PRIMITIVE_IN_FORBIDDEN_SCOPE where a leaf
// owner's callback reads it). Chrome's truth only: this program is covered on every build (`covered-uncertifiable`, the oracle's entry hands every export to `render`, so B3, ADR 0967,
// keeps a row at each site below). The position where the owner-call universe ended the call silently on main, on #220's head and on the first build of B2 is a program nothing
// hands to a package: fixtures/reactive-ir/owner-call-jsx-getter, `ByCall` (a component only a tag renders, where the owner graph gives an owner on every entry).

function Outer(props: { slot?: unknown }) {
  createEffect(
    () => 0,
    () => {
      void props.slot;
    },
  );
  return <p>outer</p>;
}

function Leaf(props: { slot?: unknown }) {
  createTrackedEffect(() => {
    void props.slot;
  });
  return <p>leaf</p>;
}

// One export each: an error thrown on an apply halts the page before another effect's callback runs.
export function ByCall() {
  return <Outer slot={createMemo(() => 1)} />;
}

export function LeafByCall() {
  return <Leaf slot={createMemo(() => 1)} />;
}

// A function the getter's own evaluation calls: the call runs where the component reads the prop too.
export function ByMap() {
  return <Outer slot={[1].map(() => createMemo(() => 1))} />;
}

// Control: the component reads the prop in its body, under the owner its own render sets; Chrome is quiet.
function InBody(props: { slot?: unknown }) {
  return <p>{String(props.slot)}</p>;
}
export function Rendered() {
  return <InBody slot={createMemo(() => 1)} />;
}
