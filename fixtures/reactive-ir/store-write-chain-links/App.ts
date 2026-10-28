import { createStore } from "solid-js";

type State = {
  v: number;
  user: { name: string };
  items: number[];
  rows: { done: boolean }[];
  opt?: { b: number };
  list?: number[];
  maybe: { b: number } | null;
};
const [store] = createStore<State>({ v: 0, user: { name: "a" }, items: [], rows: [{ done: false }], maybe: null });
const [list] = createStore<number[]>([]);

// Violation: the write reads no link past the root, the proxy itself.
export function SingleLink() {
  store.v = 1;
}

export function RootArray() {
  list.push(1);
}

// Uncertifiable: each write reads a link whose type excludes `null` and
// `undefined`, which proves nothing at run time without strictNullChecks.
export function NestedRecord() {
  store.user.name = "b";
}

export function NestedArray() {
  store.items.push(1);
}

export function ElementLink() {
  store.rows[0].done = true;
}

// `!` and `as` silence TypeScript; the links hold `null` and `undefined`
// here, so the write throws `TypeError` before reaching the proxy.
export function AssertedLink() {
  store.maybe!.b = 1;
}

export function CastLink() {
  (store.opt as { b: number }).b = 2;
}
