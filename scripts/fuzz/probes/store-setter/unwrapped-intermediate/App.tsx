// docs/open-work.md (past the root): an intermediate the store does not wrap (a frozen object) is reported although the write lands.
import { createStore } from "solid-js";

export function C1() {
  const [store] = createStore({ box: Object.freeze({ inner: { n: 0 } }) });
  return (
    <button onClick={() => {
        store.box.inner.n = 1;
      }}>{store.box.inner.n}</button>
  );
}
