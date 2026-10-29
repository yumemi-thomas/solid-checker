// The same as deferred-microtask through setTimeout.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  return (
    <div>
      <button onClick={() => {
          setStore(() => {
            setTimeout(() => {
              store.n = 1;
            }, 0);
          });
          setTimeout(() => { document.getElementById("w")!.textContent = String(store.n); }, 20);
        }}>go</button>
      <output id="w">-</output>
    </div>
  );
}
