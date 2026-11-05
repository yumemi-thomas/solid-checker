const log = (...a: any[]) => (window as any).__log(...a);
import { createStore, untrack } from "solid-js";
import { mutate } from "fz-pkg";
export function App() {
  const [st] = createStore({ a: 1 });
  mutate(st);
  log("a=" + untrack(() => st.a));
  return <p>x</p>;
}
