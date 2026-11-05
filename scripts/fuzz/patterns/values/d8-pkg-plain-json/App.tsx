const log = (...a: any[]) => (window as any).__log(...a);
import { createStore } from "solid-js";
import { dump } from "fz-pkg";
export function App() {
  const [st] = createStore({ a: 1 });
  const r = dump(st);
  log("r=" + r);
  return <p>{r}</p>;
}
