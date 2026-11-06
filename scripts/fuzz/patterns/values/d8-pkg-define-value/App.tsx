const log = (...a: any[]) => (window as any).__log(...a);
import { api } from "fz-pkg";
export function App() {
  const r = api.v;
  log("r=" + r);
  return <p>{r}</p>;
}
