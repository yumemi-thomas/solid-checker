// The same as deferred-microtask through setTimeout.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  return (
    <button onClick={() => {
        setStore(() => {
          setTimeout(() => {
            store.n = 1;
          }, 0);
        });
      }}>{store.n}</button>
  );
}
