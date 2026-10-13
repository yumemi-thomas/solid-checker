import { createSignal } from "solid-js";

export const [count] = createSignal(0);

export function sink(value: unknown) {
  return value;
}

export function fail(message: string): never {
  throw new Error(message);
}

// ---- Gets on every completion of the call, and nothing else.

export function readsA(o: { a: number }) {
  return o.a;
}
export const readsArrow = (o: { a: number }) => o.a;
export function readsThenB(o: { a: number; b: number }) {
  const x = o.b;
  return x + o.a;
}
export function readsOrZero(o: { a?: number }) {
  return o.a ?? 0;
}
export function readsB(o: { a?: number; b: number }) {
  return o.b;
}
export function readsFn(o: { a: () => number }) {
  return o.a;
}
export function callsCallback(f: () => number) {
  return f();
}

// ---- The Get is not proven on every completion, or the parameter is
// not only read.

export function readsBranch(flag: boolean, o: { a: number }) {
  if (flag) return o.a;
  return 0;
}
export function readsClosure(o: { a: number }) {
  return () => o.a;
}
export function keeps(o: { a: number }) {
  sink(o);
  return o.a;
}
export function aliases(o: { a: number }) {
  const p = o;
  return p.a;
}
export function destructures({ a }: { a: number }) {
  return a;
}
export function writesParam(o: { a: number }) {
  o = { a: 1 };
  return o.a;
}
export function viaComputed(o: { a: number }, k: "a") {
  return o[k];
}
export function viaOptional(o: { a: number } | undefined) {
  return o?.a;
}
export async function readsAsync(o: { a: number }) {
  return o.a;
}
export function* readsGenerator(o: { a: number }) {
  yield o.a;
}
export function readsDefault(o: { a: number }, x = o.a) {
  return x;
}
export function readsJsx(o: { a: number }) {
  return <div>{o.a}</div>;
}
export function guardThrows(o: { a: number }) {
  fail("always");
  return o.a;
}
export function writesMember(o: { a: number }) {
  o.a = 1;
  return 0;
}
export function deletesA(o: { a?: number; b: number }) {
  delete o.a;
  return o.b;
}
export function usesArguments(o: { a: number }) {
  return arguments[0].a + o.a;
}
export function methodCall(o: { a: number; m(): number }) {
  o.m();
  return o.a;
}
export function readsStrict(o: { a: number }) {
  "use strict";
  return o.a;
}
export function forwards(o: { a: number }) {
  return readsA(o);
}
export let readsReplaceable = (o: { a: number }) => o.a;
export function replace() {
  readsReplaceable = () => 0;
}
export class Holder {
  value: number;
  constructor(o: { a: number }) {
    this.value = o.a;
  }
}

// A module-level receiver may be read or changed by any importer.
export const sharedOptions = {
  get a() {
    return count();
  },
};
