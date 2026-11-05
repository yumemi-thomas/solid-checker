import { createStore, untrack } from "solid-js";

// An uninitialized derive's every trap throws.
export function Pending() {
  const [store] = createStore(async () => ({ a: 1 }), { a: 0 });
  const one = Object.keys(store);
  const two = "a" in store;
  const three = Object.hasOwn(store, "a");
  const four = untrack(() => Object.keys(store));
  for (const key in store) {
    console.log(key);
  }
  return <p>{[one, two, three, four].length}</p>;
}

// A settled store reads no key.
export function Settled() {
  const [store] = createStore({ a: 1 });
  const one = Object.keys(store);
  const two = "a" in store;
  return <p>{[one, two].length}</p>;
}
