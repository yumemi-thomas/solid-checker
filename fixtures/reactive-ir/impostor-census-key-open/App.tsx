import { createSignal } from "solid-js";

const [, setW] = createSignal(0);
const [items] = createSignal<number[]>([1]);
const [pending] = createSignal(Promise.resolve());

// ADR 0903: each computed key names no census member, or the value is no
// function: a number, a template or concatenation with a literal head or tail
// no member has, a copy of a literal's own members.
declare const any: any;
const table: any = {};
for (let i = 0; i < 3; i++) table[i] = () => {};
const base = 4;
table[base * 2] = () => {};
table[base + 1] = () => {};
table[Math.floor(any)] = () => {};
table[`row-${any}`] = () => {};
table["col-" + any] = () => {};
const defaults = { width: 1, onResize: () => {} };
for (const key of Object.keys(defaults)) table[key] = (defaults as any)[key];
for (const [key, value] of Object.entries(defaults)) table[key] = value;
table[any] = new Array(2);
table[any] = { a: 1, b: [2] };
export const cache: any = {};
cache.map = new Map();

export function App() {
  items().forEach(() => { setW(1); });
  pending().then(() => { setW(2); });
  [1, 2].forEach(() => { setW(3); });
  return <p />;
}
