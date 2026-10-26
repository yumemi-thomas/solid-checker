// oscartbeaumont-website routes/invoicer/util.ts:34,37: the closure that writes is what a `get` trap returns, and the trap is a member of
// the handler object of `new Proxy(...)`, so the analysis never sees where it is called.
import { createStore } from "solid-js";

export function createListProxy(initial: string[]) {
  const [items, setItems] = createStore(initial);
  return new Proxy(items, {
    get(target, key) {
      if (key === "push") return (value: string) => setItems((draft) => { draft.push(value); });
      return Reflect.get(target, key);
    },
  });
}
