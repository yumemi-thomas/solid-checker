import { onCleanup } from "solid-js";

const done = () => {};
const items: (() => void)[] = [];

// Called in the body (an owned edge), and entered another way: the click runs it with no owner.
export function Assigned() {
  const f = () => onCleanup(done);
  f();
  const el = document.createElement("button");
  el.onclick = f;
  return <i />;
}
export function Stored() {
  const g = () => onCleanup(done);
  g();
  items.push(g);
  return <button onClick={() => items[0]()} />;
}
export function Member() {
  const slot = { run: () => {} };
  const h = () => onCleanup(done);
  h();
  slot.run = h;
  return <i />;
}
// Reviewer D13: a value-only argument may keep it; an object slot has no binding; a layer
// that only may be a component is no component.
export function KeptThis() {
  const keep = function (this: () => void) { items.push(this); };
  const n = () => onCleanup(done);
  n();
  keep.call(n);
  return <i />;
}
export function Slot() {
  const o = { run: () => onCleanup(done) };
  o.run();
  const el = document.createElement("button");
  el.onclick = o.run;
  return <i />;
}
export function Row() {
  const row = () => { onCleanup(done); return <i />; };
  row();
  const el = document.createElement("button");
  el.onclick = () => { row(); };
  return <i />;
}
// A `removeEventListener` argument: nothing proves the callee drops the value.
export function Removed() {
  const m = () => onCleanup(done);
  m();
  document.body.removeEventListener("click", m);
  return <i />;
}

// Every entry is a call on the body's stack, or an inline run there: owned, silent.
export function Twice() {
  const k = () => onCleanup(done);
  k();
  k();
  return <i />;
}
export function Inline() {
  const p = () => onCleanup(done);
  [1].forEach(p);
  return <i />;
}
