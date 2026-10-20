import { createSignal } from "solid-js";

export type Provider = { query: (input: string) => number };
export function makeProviders(): Provider[] {
  const [first] = createSignal(1);
  const [second] = createSignal(2);
  const queryFirst = (_input: string) => { return first(); };
  const querySecond = (_input: string) => { return second(); };
  return [{ query: queryFirst }, { query: querySecond }];
}
export function makePlain(): Provider[] {
  return [{ query: (_input: string) => { return 0; } }];
}
export function makeAsync(): { query: () => Promise<number> }[] {
  const [count] = createSignal(1);
  const query = async () => { await Promise.resolve(); return count(); };
  return [{ query }];
}
export function makeOpen(): Provider[] {
  const [count] = createSignal(1);
  const query = (_input: string) => { return count(); };
  const extra = { id: "extra" };
  return [{ query, ...extra }];
}
export function makeAlternative(flag: boolean): Provider[] {
  if (flag) return makeProviders();
  return makePlain();
}
export function makeInline(): Provider[] {
  const [count] = createSignal(1);
  return [{ query: (_input: string) => count() }];
}
export function makeGenerator() {
  const [count] = createSignal(1);
  function* query() { yield count(); }
  return [{ query }];
}
export function makeMethod() {
  const [count] = createSignal(1);
  return [{ query() { return count(); } }];
}
export function makeHoles() {
  const [count] = createSignal(1);
  const query = (_input: string) => { return count(); };
  return [{ query }, ,];
}
export function makeSpread(): Provider[] {
  const [count] = createSignal(1);
  const query = (_input: string) => { return count(); };
  const more = [{ query }];
  return [{ query }, ...more];
}

import type { SourceAccessor } from "solid-js";
declare const declared: SourceAccessor<number>;
export function makeTyped(): Provider[] {
  const query = (_input: string) => { return declared(); };
  return [{ query }];
}
