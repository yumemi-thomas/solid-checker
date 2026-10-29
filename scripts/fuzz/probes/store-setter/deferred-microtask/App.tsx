// A write inside setStore's callback that runs after the setter returns (a microtask): the draft is closed, so rc.17 discards it
// and reports STORE_WRITE_OUTSIDE_SETTER. The checker's own-setter exemption is lexical, so it is silent (Chrome-confirmed 2026-10-28).
// Witness: #w shows the store's raw value 20 ms later; "1" would mean the write landed.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  return (
    <div>
      <button onClick={() => {
          setStore(() => {
            queueMicrotask(() => {
              store.n = 1;
            });
          });
          setTimeout(() => { document.getElementById("w")!.textContent = String(store.n); }, 20);
        }}>go</button>
      <output id="w">-</output>
    </div>
  );
}
