// Open question: a write through the read proxy (not the draft) inside the store's own setter callback, synchronously.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  return (
    <button onClick={() => {
        setStore(() => {
          store.n = 1;
        });
      }}>{store.n}</button>
  );
}
