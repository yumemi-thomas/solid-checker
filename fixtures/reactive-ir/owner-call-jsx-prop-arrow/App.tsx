import { createEffect, createMemo, createTrackedEffect } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, B2 (the review of #225, F2): the body of an arrow written in a component element's prop is run by whoever calls it, not where a getter is read. A component the
// project writes can call it on an effect's apply (the tag it creates has no owner there: rc.21 PRIMITIVE_IN_EFFECT_CALLBACK, SC6002) or in a leaf owner's callback
// (PRIMITIVE_IN_FORBIDDEN_SCOPE, SC3001). The owner-call universe skipped every JSX written inside a component element's span, the arrow's body included, so the program certified.

function Inner() {
  return <p>inner</p>;
}

// The apply calls the arrow through `props.view()`.
function Outer(props: { view: () => unknown }) {
  createEffect(
    () => 0,
    () => {
      props.view();
    },
  );
  return <p>outer</p>;
}

// A leaf owner's callback calls it.
function Leaf(props: { view: () => unknown }) {
  createTrackedEffect(() => {
    props.view();
  });
  return <p>leaf</p>;
}

function ByPropArrow() {
  return (
    <p>
      <Outer
        view={() => (
          <Inner />
        )}
      />
      <Leaf
        view={() => (
          <Inner />
        )}
      />
    </p>
  );
}

// The arrow's body is a plain function body, so a JSX written in its block is a site too.
function ByBlockArrow() {
  return (
    <Outer
      view={() => {
        return <Inner />;
      }}
    />
  );
}

// A creating call written in the same place is a proven violation already, before this change as after (the effect-phase walk follows `props.view()` from the apply to the arrow,
// ADR 0883): the arrow's body is one place whatever it holds. A JSX creation there is accounted for, never proven.
function ByCall() {
  return (
    <Outer
      view={() => {
        createMemo(() => 1);
      }}
    />
  );
}

// A component that calls the arrow in its body: the call runs under the owner the component's own render sets, but nothing the checker reads proves it is the only caller, so
// this is a row too (the same as a creating call there).
function InBody(props: { view: () => unknown }) {
  return <p>{props.view()}</p>;
}
function Rendered() {
  return (
    <InBody
      view={() => (
        <Inner />
      )}
    />
  );
}

// A component that never reads the prop: the arrow never runs, which the checker does not prove for an arrow handed to a component's prop (a row, as for a creating call).
function Ignores(_props: { view: () => unknown }) {
  return <p>ignores</p>;
}
function Unread() {
  return (
    <Ignores
      view={() => (
        <Inner />
      )}
    />
  );
}

function App() {
  return (
    <p>
      <ByPropArrow />
      <ByBlockArrow />
      <ByCall />
      <Rendered />
      <Unread />
    </p>
  );
}
render(() => <App />, document.body);
