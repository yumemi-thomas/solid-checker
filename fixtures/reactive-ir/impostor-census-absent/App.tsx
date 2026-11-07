import { createSignal } from "solid-js";

const [, setW] = createSignal(0);
const [items] = createSignal<number[]>([1]);
const [pending] = createSignal(Promise.resolve());

// None of these puts a function at a census member: a data literal, a key a
// `const` names, another name, a type, a field with no function in it.
const KEY = "other";
export const data = { then: 1, forEach: "x" };
export const named = { [KEY]: () => {} };
export const near = { thenable() {}, forEachOf() {} };
export interface Shape { then(f: () => void): void }
export class Plain { then?: number; forEach = 0; }

export function App() {
  items().forEach(() => { setW(1); });
  pending().then(() => { setW(2); });
  return <p />;
}
