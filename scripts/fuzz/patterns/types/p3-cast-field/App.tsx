import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
class H { p = { then(f: () => void) { f(); } } as unknown as Promise<void>; }
export function App() {
  new H().p.then(() => { setW(1); });
  return <p />;
}
