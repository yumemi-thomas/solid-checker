import { createEffect, createTrackedEffect } from "solid-js";

// B2 (the review of #225, F2): a component element's children are a getter that runs where the component reads `props.children`. Read on an effect's apply, the getter's tag
// `<Inner />` is created on a stack with no owner: Chrome rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK there (and PRIMITIVE_IN_FORBIDDEN_SCOPE where a leaf owner's callback reads it).
// The owner-call universe skipped the tag as a getter's, "read where the component reads it", and no row stood at the read for these codes.

function Inner() {
  return <p>inner</p>;
}

function Outer(props: { children?: any }) {
  createEffect(
    () => 0,
    () => {
      void props.children;
    },
  );
  return <p>outer</p>;
}

function Leaf(props: { children?: any }) {
  createTrackedEffect(() => {
    void props.children;
  });
  return <p>leaf</p>;
}

export function ByGetter() {
  return (
    <Outer>
      <Inner />
    </Outer>
  );
}

// A leaf owner's callback reads it. One export each: an error thrown on an apply halts the page before the other effect's callback runs.
export function LeafByGetter() {
  return (
    <Leaf>
      <Inner />
    </Leaf>
  );
}

// Control: the component reads its children in its body, under the owner its own render sets; Chrome is quiet.
function InBody(props: { children?: any }) {
  return <p>{props.children}</p>;
}

export function Rendered() {
  return (
    <InBody>
      <Inner />
    </InBody>
  );
}
