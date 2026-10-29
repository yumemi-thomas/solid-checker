// A write through the read proxy (not the draft) inside the store's own setter callback, synchronously: it lands (Chrome, 2026-10-28).
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  return (
    <div>
      <button onClick={() => {
          setStore(() => {
            store.n = 1;
          });
          setTimeout(() => { document.getElementById("w")!.textContent = String(store.n); }, 20);
        }}>go</button>
      <output id="w">-</output>
    </div>
  );
}
