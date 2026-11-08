import { createStore } from "solid-js";
import { markRaw } from "@solidjs/signals";

declare const rec: { a: number };

// With a `markRaw` or a shallow store in the project, a store over a value no
// literal states may be that raw value: no read, and a write lands.
export function Raw() {
  const [marked] = createStore(markRaw({ a: 1 }));
  const [held] = createStore(rec);
  const one = marked.a;
  const two = held.a;
  return (
    <button
      onClick={() => {
        marked.a = 2;
      }}
    >
      {[one, two].length}
    </button>
  );
}

// A literal is wrapped: the read warns.
export function Wrapped() {
  const [store] = createStore({ a: 1 });
  const one = store.a;
  return <p>{one}</p>;
}
