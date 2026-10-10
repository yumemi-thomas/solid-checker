import { createMemo, createSignal, createStore } from "solid-js";
import { evens, parse, same } from "./source";

// Clean dispatch: every value `items` holds is an array built here -- the
// initial literal, a spread, a filter of the previous value -- so `evens`'s
// `filter` is `Array.prototype.filter`. (The read itself is the body's.)
export function UpdatedSignal() {
  const [items, setItems] = createSignal([1, 2]);
  const add = () => setItems((previous) => [...previous, previous.length]);
  const drop = () => setItems((previous) => previous.filter((value) => value > 1));
  const reset = () => setItems([]);
  const picked = evens(items());
  return <div onClick={() => (add(), drop(), reset())}>{picked.length}</div>;
}

// Clean dispatch: a memo whose compute returns a fresh, sorted copy.
export function SortedMemo() {
  const [items] = createSignal([3, 1, 2]);
  const sorted = createMemo(() => [...items()].sort());
  const picked = evens(sorted());
  return <div>{picked.length}</div>;
}

// Uncertifiable: the setter is handed on, so anything may be written.
export function EscapedSetter() {
  const [items, setItems] = createSignal([1, 2]);
  const handlers = { setItems };
  const picked = evens(items());
  return <div onClick={() => handlers.setItems([3])}>{picked.length}</div>;
}

// Uncertifiable: a store's array is written into the signal.
export function StoreWritten() {
  const [state] = createStore({ items: [1, 2] });
  const [items, setItems] = createSignal([0]);
  const pick = () => setItems(state.items);
  const picked = evens(items());
  return <div onClick={pick}>{picked.length}</div>;
}

// Uncertifiable: `createSignal(fn)` is a writable memo.
export function WritableMemo() {
  const [state] = createStore({ items: [1, 2] });
  const [items] = createSignal(() => state.items);
  const picked = evens(items());
  return <div>{picked.length}</div>;
}

// Uncertifiable: a memo that returns a store's array.
export function StoreMemo() {
  const [state] = createStore({ items: [1, 2] });
  const items = createMemo(() => state.items);
  const picked = evens(items());
  return <div>{picked.length}</div>;
}

// Clean dispatch: a derived function and a helper that return fresh arrays,
// and a fallback over a memo that is an array or nothing.
export function DerivedValues() {
  const [items] = createSignal([3, 1, 2]);
  const [flag] = createSignal(true);
  const sorted = () => [...items()].sort();
  const maybe = createMemo(() => (flag() ? [1] : undefined));
  const picked = evens(sorted()).concat(evens(parse("1,2")), evens(maybe() ?? []));
  return <div>{picked.length}</div>;
}

// Uncertifiable: `same` returns its parameter, whatever its caller passed.
export function ReturnedParameter() {
  const [state] = createStore({ items: [1, 2] });
  const picked = evens(same(state.items));
  return <div>{picked.length}</div>;
}
