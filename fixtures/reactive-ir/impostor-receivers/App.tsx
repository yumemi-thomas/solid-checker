import { createMemo, createSignal } from "solid-js";

const late = (cb: () => void) => { queueMicrotask(cb); };

// The callback's third parameter is the array.
const handed = [1];
handed.map((_v, _i, a) => { (a as any).forEach = late; return 0; });

const reduced = [1, 2];
reduced.reduce((c, _v, _i, a) => { (a as any).forEach = late; return c; }, 0);

// A method of the prototype gets the array as `this`.
(Array.prototype as any).hand = function (this: any) { this.forEach = late; };
const held = [1];
(held as any).hand();

// A named callback may declare the third parameter.
const steal = (_v: number, _i: number, a: number[]) => { (a as any).forEach = late; };
const named = [1];
named.map(steal);

export function HandedOn() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => { handed.forEach(() => { set(1); }); return s(); });
  return <p>{m()}</p>;
}
export function ReduceHandedOn() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => { reduced.forEach(() => { set(1); }); return s(); });
  return <p>{m()}</p>;
}
export function ThisHandedOn() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => { held.forEach(() => { set(1); }); return s(); });
  return <p>{m()}</p>;
}
export function NamedCallback() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => { named.forEach(() => { set(1); }); return s(); });
  return <p>{m()}</p>;
}

// A listed method the program replaced runs with the array as `this`.
const slicedOwn = Array.prototype.slice;
(Array.prototype as any).slice = function (this: any, ...a: any[]) { this.forEach = late; return slicedOwn.apply(this, a); };
const sliced = [1];
sliced.slice();

export function PatchedListed() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => { sliced.forEach(() => { set(1); }); return s(); });
  return <p>{m()}</p>;
}
