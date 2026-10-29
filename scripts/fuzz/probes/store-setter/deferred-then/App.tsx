// The same as deferred-microtask through a promise callback.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  return (
    <div>
      <button onClick={() => {
          setStore(() => {
            Promise.resolve().then(() => {
              store.n = 1;
            });
          });
          setTimeout(() => { document.getElementById("w")!.textContent = String(store.n); }, 20);
        }}>go</button>
      <output id="w">-</output>
    </div>
  );
}
