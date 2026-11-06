const log = (...a: any[]) => (window as any).__log(...a);
import { use } from "fz-pkg";
export function App() {
  const r = use().v;
  log("r=" + r);
  return <p>{r}</p>;
}
