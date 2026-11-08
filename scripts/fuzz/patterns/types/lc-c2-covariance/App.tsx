import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
type Thenable = { then(f: () => void): void };
const ps: Promise<void>[] = [];
const wide: (Promise<void> | Thenable)[] = ps;
wide.push({ then(f: () => void) { f(); } });
export function App() {
  ps[0].then(() => { setW(1); });
  return <p>{w()}</p>;
}
