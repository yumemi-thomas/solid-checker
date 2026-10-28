// docs/open-work.md (store write outside a setter): a closure defined outside setStore(...) and called inside its callback commits on
// rc.17, but the lexical exemption does not see it, so it is reported: a documented false positive.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  const bump = () => {
    store.n = 1;
  };
  return (
    <button onClick={() => {
        setStore(() => {
          bump();
        });
      }}>{store.n}</button>
  );
}
