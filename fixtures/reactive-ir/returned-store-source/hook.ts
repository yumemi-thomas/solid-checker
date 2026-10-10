import { createSignal, createStore } from "solid-js";
import * as Solid from "solid-js";

export type LocationShape = { search: string; items: string[] };
const plain = (): LocationShape => ({ search: "?plain", items: ["one"] });

export function useLocation() {
  const [location] = createStore({ search: "?plan=pro", items: ["one"] });
  return location;
}
export { useLocation as exportedAlias };
export const arrowLocation = () => {
  const [location] = Solid.createStore({ search: "?plan=pro", items: ["one"] });
  return location;
};
export function useCount() {
  const [count] = createSignal(0);
  return count;
}
export function sameSource(flag: boolean) {
  const [state] = createStore(plain());
  if (flag) return state;
  return state;
}
export function exhaustiveSource(flag: boolean) {
  const [state] = createStore(plain());
  if (flag) return state;
  else return state;
}
export function wrappedSource() {
  const [state] = createStore(plain());
  return (state as LocationShape);
}
export function differentSources(flag: boolean) {
  const [left] = createStore(plain());
  const [right] = createStore(plain());
  if (flag) return left;
  return right;
}
export function otherReturn(flag: boolean) {
  const [state] = createStore(plain());
  if (flag) return state;
  return plain();
}
export function implicitUndefined(flag: boolean) {
  const [state] = createStore(plain());
  if (flag) return state;
}
export function explicitUndefined(flag: boolean) {
  const [state] = createStore(plain());
  if (flag) return;
  return state;
}
export async function asyncSource() {
  const [state] = createStore(plain());
  return state;
}
export function* generatorSource(): Generator<never, LocationShape, unknown> {
  const [state] = createStore(plain());
  return state;
}
export function writtenSource() {
  let [state] = createStore(plain());
  state = plain();
  return state;
}
function identity<T>(value: T): T { return value; }
export function passedSource() {
  const [state] = createStore(plain());
  return identity(state);
}
export function escapedSource() {
  const [state] = createStore(plain());
  identity(state);
  return state;
}
export function aliasedSource() {
  const [state] = createStore(plain());
  const alias = state;
  identity(alias);
  return state;
}
export function memberSource() {
  const [state] = createStore(plain());
  return state.items;
}
export function finallyOverrides() {
  const [state] = createStore(plain());
  try { return state; }
  finally { return plain(); }
}
export function loopSource(flag: boolean) {
  const [state] = createStore(plain());
  while (flag) return state;
  return state;
}
export function capturedSource() {
  return outside;
}
const [outside] = createStore(plain());
export function typedPlain(): LocationShape { return plain(); }
export declare function unresolvedHook(): LocationShape;
export let reassignedHook = (): LocationShape => {
  const [state] = createStore(plain());
  return state;
};
reassignedHook = () => plain();
