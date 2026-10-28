// Control: the draft write inside the setter is the legal form. Solid is silent and so must the checker be.
import { createStore } from "solid-js";

export function C1() {
  const [store, setStore] = createStore({ n: 0 });
  return (
    <button onClick={() => {
        setStore((draft) => {
          draft.n = 1;
        });
      }}>{store.n}</button>
  );
}
