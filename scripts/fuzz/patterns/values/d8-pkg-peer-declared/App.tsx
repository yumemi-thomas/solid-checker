const log = (...a: any[]) => (window as any).__log(...a);
import { read } from "fz-pkg";
export function App() {
  const r = read();
  log("r=" + r);
  return <p>{r}</p>;
}
