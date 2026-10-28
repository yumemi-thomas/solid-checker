// Control: the draft write inside the setter is the legal form. Solid is silent and so must the checker be; the write lands.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  return (
    <div>
      <button onClick={() => {
          setStore((draft) => {
            draft.n = 1;
          });
          setTimeout(() => { document.getElementById("w")!.textContent = String(store.n); }, 20);
        }}>go</button>
      <output id="w">-</output>
    </div>
  );
}
