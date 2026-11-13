import { createEffect, createTrackedEffect } from "solid-js";

// B2 (the review of #225, F2): the body of an arrow written in a component element's prop is run by whoever calls it, not where a getter is read. Chrome rc.21 throws
// PRIMITIVE_IN_EFFECT_CALLBACK at the tag `<Inner />` that `Outer`'s effect apply evaluates through `props.view()` (createComponent runs its component under createRoot, which
// refuses an apply's stack with no owner), and PRIMITIVE_IN_FORBIDDEN_SCOPE at the tag a leaf owner's callback evaluates. The owner-call universe skipped every JSX written inside a
// component element's span, the arrow's body included, so the checker certified both.

function Inner() {
  return <p>inner</p>;
}

function Outer(props: { view: () => any }) {
  createEffect(
    () => 0,
    () => {
      props.view();
    },
  );
  return <p>outer</p>;
}

function Leaf(props: { view: () => any }) {
  createTrackedEffect(() => {
    props.view();
  });
  return <p>leaf</p>;
}

export function ByPropArrow() {
  return (
    <Outer
      view={() => (
        <Inner />
      )}
    />
  );
}

// A leaf owner's callback calls the arrow. One export each: an error thrown on an apply halts the page before the other effect's callback runs.
export function LeafByPropArrow() {
  return (
    <Leaf
      view={() => (
        <Inner />
      )}
    />
  );
}

// Control: the component calls the arrow in its body, under the owner its own render sets; Chrome is quiet.
function InBody(props: { view: () => any }) {
  return <p>{props.view()}</p>;
}

export function Rendered() {
  return (
    <InBody
      view={() => (
        <Inner />
      )}
    />
  );
}
