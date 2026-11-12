import { createEffect, createTrackedEffect } from "solid-js";

// B2 (the re-review of #220): JSX is a creation as a call is. The program of fixtures/reactive-ir/owner-call-jsx-sites with each function a reference enters at MODULE scope (the
// oracle's entry hands every export to `render` of the real `@solidjs/web`, so B3, ADR 0967, keeps a row at every site written inside an exported component on every build; a
// function nothing but a module-scope initializer names has no function around its reference). Chrome rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK at the tag `<Inner />` an effect's
// apply evaluates through the reference (createComponent runs its component under createRoot, which refuses an apply's stack with no owner), and PRIMITIVE_IN_FORBIDDEN_SCOPE
// at the tag a leaf owner's callback evaluates. The checker used to certify both: the owner-call universe listed creating calls only, and no call is written here.

function Inner() {
  return <p>inner</p>;
}

function view() {
  return <Inner />;
}
const pick: (() => unknown) | undefined = view;
export function ByReference() {
  createEffect(
    () => 0,
    () => {
      pick!();
    },
  );
  return <p>by reference</p>;
}

function viewLeaf() {
  return <Inner />;
}
const pickLeaf: (() => unknown) | undefined = viewLeaf;
export function LeafByReference() {
  createTrackedEffect(() => {
    pickLeaf!();
  });
  return <p>leaf by reference</p>;
}

// Control: a component only a tag renders; Chrome is quiet.
export function Rendered() {
  return (
    <p>
      <Inner />
    </p>
  );
}
