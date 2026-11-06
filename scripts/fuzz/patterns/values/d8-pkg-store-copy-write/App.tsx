const log = (...a: any[]) => (window as any).__log(...a);
import { value } from "fz-pkg";
export function App() {
  value.n = 5;
  return <p>x</p>;
}
