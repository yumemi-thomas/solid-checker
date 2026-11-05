const log = (...a: any[]) => (window as any).__log(...a);
import { value } from "fz-pkg";
import { untrack } from "solid-js";
export function App() {
  value.n = 5;
  const r = untrack(() => value.n);
  log("r=" + r);
  return <p>{r}</p>;
}
