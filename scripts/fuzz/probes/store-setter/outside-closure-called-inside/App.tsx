// docs/open-work.md (store write outside a setter): a closure defined outside setStore(...) and called inside its callback commits on
// rc.17, but the lexical exemption does not see it, so it is reported: a documented false positive. Witness: "1" means it landed.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  const bump = () => {
    store.n = 1;
  };
  return (
    <div>
      <button onClick={() => {
          setStore(() => {
            bump();
          });
          setTimeout(() => { document.getElementById("w")!.textContent = String(store.n); }, 20);
        }}>go</button>
      <output id="w">-</output>
    </div>
  );
}
