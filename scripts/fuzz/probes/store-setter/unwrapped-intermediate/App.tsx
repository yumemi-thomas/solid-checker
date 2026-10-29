// docs/open-work.md (past the root): an intermediate the store does not wrap (a frozen object) is reported although the write lands.
// Witness: the raw value of the nested field, "1" when the write landed.
import { createStore } from "solid-js";

export function C1() {
  const [store] = createStore({ box: Object.freeze({ inner: { n: 0 } }) });
  return (
    <div>
      <button onClick={() => {
          store.box.inner.n = 1;
          setTimeout(() => { document.getElementById("w")!.textContent = String(store.box.inner.n); }, 20);
        }}>go</button>
      <output id="w">-</output>
    </div>
  );
}
