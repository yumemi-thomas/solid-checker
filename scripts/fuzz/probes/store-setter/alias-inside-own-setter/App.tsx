// A write through an exact const alias of the store inside its own setter callback: the alias is the same proxy, so it commits
// (Chrome, 2026-10-28: no diagnostic, the value reads back 1). Found by legal.mjs; reported as a violation before the fix.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  const alias = store;
  return (
    <div>
      <button onClick={() => {
          setStore(() => {
            alias.n = 1;
          });
          setTimeout(() => { document.getElementById("w")!.textContent = String(store.n); }, 20);
        }}>go</button>
      <output id="w">-</output>
    </div>
  );
}
