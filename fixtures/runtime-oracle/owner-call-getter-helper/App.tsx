import { createEffect, createMemo, createTrackedEffect } from "solid-js";

// B2 (the review of #225, F2, found while fixing it): a CALL written in a component element's getter runs its callee where the component reads the prop. `renderView` and
// `makeMemo` are written outside the element and called inside its prop; the component reads the prop on an effect's apply (no owner set: rc.21 PRIMITIVE_IN_EFFECT_CALLBACK at
// the tag and at the memo) or in a leaf owner's callback (PRIMITIVE_IN_FORBIDDEN_SCOPE). The analyses placed the callee by the position of its call, a component's render, and
// ended the sites silently on main and on #220's head. One export each: an error thrown on an apply halts the page before another effect's callback runs. In this program every
// export is handed (the oracle's entry renders them), so B3 may cover a callee an export's getter calls on every build; the silent position is fixtures/reactive-ir/owner-call-getter-helper.

function Inner() {
  return <p>inner</p>;
}

function renderView() {
  return <Inner />;
}

function makeMemo() {
  return createMemo(() => 1);
}

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

export function ByHelper() {
  return <Outer slot={renderView()} />;
}

export function ByMakeMemo() {
  return <Outer slot={makeMemo()} />;
}

export function LeafByHelper() {
  return <Leaf slot={renderView()} />;
}

// Control: the helper called in the component's own render, under the owner its render sets; Chrome is quiet.
function controlView() {
  return <Inner />;
}
export function Rendered() {
  return <p>{controlView()}</p>;
}
