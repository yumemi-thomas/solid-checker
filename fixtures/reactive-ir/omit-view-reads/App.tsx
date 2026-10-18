import { createStore, merge, omit } from "solid-js";

// ADR 0323: an `omit` view reads through to its props argument like a merge
// view does, minus the string-literal keys it hides. A view's reads are never
// proven violations (its keys are not the parameter's, so the caller
// classification proves nothing for them): uncertifiable at best.

function now(f: () => void) {
  f();
}

type P = { x: number; y: number; z: number };

// Read of a key the view keeps, in the body and in a callback a project callee
// runs: an uncertifiable obligation.
export function Direct(props: P) {
  const o = omit(props, "y");
  console.log(o.x);
  return <div />;
}
export function InCallback(props: P) {
  const o = omit(props, "y");
  now(() => console.log(o.x));
  return <div />;
}
// A hidden key reads nothing of the props object: silent.
export function Hidden(props: P) {
  const o = omit(props, "y", "z");
  console.log((o as Partial<P>).y);
  return <div />;
}
// Hidden through an inner view; and a key neither view hides is a read.
export function Nested(props: P) {
  const inner = omit(props, "y");
  const outer = omit(inner, "z");
  console.log((outer as Partial<P>).y, outer.x);
  return <div />;
}
// A predicate hides keys the checker cannot name: the key is still a read.
export function Predicate(props: P) {
  const o = omit(props, (key) => key === "y");
  console.log(o.y);
  return <div />;
}
// A spread of the view reads every kept key: an obligation, never a violation
// (the keys the view hides may be the only reactive ones).
export function Spread(props: P) {
  const o = omit(props, "y");
  const copy = { ...o };
  console.log(copy);
  return <div />;
}
// A merge view, for comparison: reported the same way.
export function Merged(props: P) {
  const m = merge(props, { w: 1 });
  console.log(m.x);
  return <div />;
}
// Writing through the view is dropped: the omit proxy's `set` trap is a no-op
// (`@solidjs/signals@2.0.0-rc.13` `dist/prod/store/utils.js:850`).
export function Write(props: { ref?: unknown; label: string }) {
  const o = omit(props, "label");
  o.ref = 1;
  return <div />;
}

// Review round 10. An inline view is the same expression as a bound one.
export function OmitOfOmit(props: { x: number; y: number; ref?: unknown }) {
  const o = omit(omit(props, "y"), "ref");
  console.log(o.x);
  return <div />;
}
export function InlineView(props: P) {
  console.log(omit(props, "y").x);
  return <div />;
}
export function MergeOfMerge(props: P) {
  const o = merge({ z: 1 }, merge({ w: 1 }, props));
  console.log(o.x);
  return <div />;
}
// A hidden key read through a wrapper, and an inline hidden key: silent.
export function HiddenThroughCast(props: P) {
  const o = omit(props, "y");
  console.log((o as unknown as P).y);
  return <div />;
}
export function InlineHidden(props: P) {
  console.log((omit(props, "y") as Partial<P>).y);
  return <div />;
}
// A non-literal key hides nothing nameable: a read through the view stays a
// read, and the key expression itself is evaluated in the body.
export function VariableKey(props: P & { k: "y" }) {
  const o = omit(props, props.k);
  console.log((o as unknown as P).y);
  return <div />;
}
// TypeScript wrappers on the receiver are transparent.
export function CastReceiver(props: P) {
  console.log((props as P).x, props!.y);
  return <div />;
}
// A view over a store reads the store: the same proven finding as `s.x`.
export function OmitOfStore() {
  const [s] = createStore({ x: 1, y: 2 });
  const o = omit(s, "y");
  console.log(o.x);
  return <div />;
}
export function InlineStoreView() {
  const [s] = createStore({ x: 1, y: 2 });
  console.log(omit(s, "y").x);
  return <div />;
}
export function StoreViewHidden() {
  const [s] = createStore({ x: 1, y: 2 });
  console.log((omit(s, "y") as { y?: number }).y);
  return <div />;
}
