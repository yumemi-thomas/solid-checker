import { createStore } from "solid-js";
import { Counter, Live, Patched, Ticker, bump, contains, evens, peek, run, stamp } from "./source";

declare const lookup: Record<string, number>;

// Clean: an array literal's `filter` is `Array.prototype.filter`.
export function ArrayLiteral() {
  const picked = evens([1, 2, 3]);
  return <div>{picked.length}</div>;
}

// Clean: so is a `const` bound to one, and to arrays built from it.
export function ArrayBinding() {
  const all = [1, 2, 3, 4];
  const some = all.slice(1);
  const picked = evens(some);
  return <div>{picked.length}</div>;
}

// Clean: `Array.from`, `Object.values` and `split` return fresh arrays.
export function FreshArrays() {
  const fromSet = evens(Array.from(new Set([1, 2])));
  const fromRecord = evens(Object.values(lookup));
  const fromText = evens("1,2".split(",").map(Number));
  return <div>{fromSet.length + fromRecord.length + fromText.length}</div>;
}

// Clean: a `Date` and a `Set` built here are plain built-in values.
export function BuiltinInstances() {
  const time = stamp(new Date(0));
  const today = new Date();
  const later = stamp(today);
  const found = contains(new Set(["a"]), "a");
  return <div>{time + later + Number(found)}</div>;
}

// Clean: `Counter.bump` is the method that runs, and it reads nothing.
export function ExactClass() {
  const counter = new Counter();
  const value = bump(counter);
  return <div>{value}</div>;
}

// Uncertifiable read: `Live.now` runs and reads `count`; the read reaches
// this call site, but that it runs during the call is proven only through
// plain calls (ADR 0204).
export function ExactClassRead() {
  const value = peek(new Live());
  return <div>{value}</div>;
}

// Uncertifiable: a store's array is a proxy; its `filter` reads the store.
export function StoreArray() {
  const [state] = createStore({ items: [1, 2, 3] });
  const picked = evens(state.items);
  return <div>{picked.length}</div>;
}

// Uncertifiable: a `let` may hold another value by the time of the call.
export function MutableBinding() {
  let list = [1, 2];
  const picked = evens(list);
  return <div>{picked.length}</div>;
}

// Uncertifiable: `Ticker` inherits `bump`; inherited methods are not
// followed.
export function InheritedMethod() {
  const value = bump(new Ticker());
  return <div>{value}</div>;
}

// Uncertifiable: the program assigns a `run` member, so `Patched.run` may not
// be what runs.
export function ReassignedMethod() {
  const value = run(new Patched());
  return <div>{value}</div>;
}

// Uncertifiable: a proxy's members are its handler's.
export function ProxyValue() {
  const picked = evens(new Proxy([1, 2], {}));
  return <div>{picked.length}</div>;
}
