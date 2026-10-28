// A write inside setStore's callback that runs after the setter returns (a microtask): the draft is closed, so rc.17 discards it
// and reports STORE_WRITE_OUTSIDE_SETTER. The checker's own-setter exemption is lexical, so it is silent (Chrome-confirmed 2026-10-28).
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  return (
    <button onClick={() => {
        setStore(() => {
          queueMicrotask(() => {
            store.n = 1;
          });
        });
      }}>{store.n}</button>
  );
}
