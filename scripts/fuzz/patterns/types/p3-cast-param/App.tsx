import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
function run(p: Promise<void>) { p.then(() => { setW(1); }); }
export function App() {
  const t = { then(f: () => void) { f(); } };
  run(t as any);
  return <p />;
}
