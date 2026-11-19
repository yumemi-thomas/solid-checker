import { createStore } from "solid-js";

export function Copies() {
  const [st] = createStore({ items: [{ id: 1, v: 1 }] });
  const plain = [{ id: 1, v: 1 }];
  return (
    <div>
      <button
        onClick={() => {
          // `slice` answers a copy whose elements are the store's: the write is dropped (ADR 1104, d2n).
          const c = st.items.slice();
          c[0].v = 5;
        }}
      >
        slice
      </button>
      <button
        onClick={() => {
          const c = st.items.filter(() => true);
          c[0].v = 6;
        }}
      >
        filter
      </button>
      <button
        onClick={() => {
          const c = st.items;
          c[0].v = 7;
        }}
      >
        alias
      </button>
      <button
        onClick={() => {
          // Twin: a plain array's copy holds plain objects.
          const c = plain.slice();
          c[0].v = 8;
        }}
      >
        plain
      </button>
    </div>
  );
}
