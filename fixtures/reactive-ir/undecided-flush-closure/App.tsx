// ADR 0571: in a body entered only through an exact dynamic factory (ADR
// 0530), `flush(fn)`'s callback is carved out of the undecided region (ADR
// 0542), and so is a local closure whose every reference is a plain call
// inside that callback: it runs only there. rc.17's `dynamicCore` renders the
// body under an unlabelled `untrack`, and `flush(fn)` keeps the label, so the
// store read is a stale snapshot rc.17 does not warn (ADR 0530), while the
// write throws STORE_WRITE_OUTSIDE_SETTER.
import { createStore, dynamicComponent, flush } from "solid-js";
import * as Solid from "solid-js";
import { flush as lookalikeFlush } from "./lookalike";

// Violation (SC2003) at the closure's write; SC1001 stale snapshots.
function Wrapped() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  const wrapUse = () => {
    s1.a.v = 7;
  };
  flush(() => {
    wrapUse();
  });
  return <div />;
}

// Violation (SC2003): the same through the namespace and an array write.
function Namespaced() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  const wrapPush = () => {
    s1.items.push(1);
  };
  Solid.flush(() => {
    wrapPush();
  });
  return <div />;
}

// Uncertifiable: the closure is also called in the body itself.
function AlsoInBody() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  const wrapUse = () => {
    s1.a.v = 7;
  };
  flush(() => {
    wrapUse();
  });
  wrapUse();
  return <div />;
}

// Uncertifiable: the closure is handed to `flush` as a value, not called.
function Handed() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  const wrapUse = () => {
    s1.a.v = 7;
  };
  flush(wrapUse);
  return <div />;
}

// Uncertifiable: a project `flush` lookalike (shadowed name) places nothing.
function Lookalike() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  const wrapUse = () => {
    s1.a.v = 7;
  };
  lookalikeFlush(() => {
    wrapUse();
  });
  return <div />;
}

// Uncertifiable: handed to `map` as a plain function, so the flush callback
// is not carved (ADR 0542), nor is the closure it calls.
function Row() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  const wrapUse = () => {
    s1.a.v = 7;
  };
  flush(() => {
    wrapUse();
  });
  return <div />;
}

export const DynWrapped = dynamicComponent(() => Wrapped);
export const DynNamespaced = dynamicComponent(() => Namespaced);
export const DynAlsoInBody = dynamicComponent(() => AlsoInBody);
export const DynHanded = dynamicComponent(() => Handed);
export const DynLookalike = dynamicComponent(() => Lookalike);

export function App() {
  return (
    <div>
      <DynWrapped />
      <DynNamespaced />
      <DynAlsoInBody />
      <DynHanded />
      <DynLookalike />
      {[1, 2].map(Row)}
    </div>
  );
}
