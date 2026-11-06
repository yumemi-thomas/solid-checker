import { createSignal } from "solid-js";
function run(o: { go: (v: number) => void }) { o.go(1); return 1; }
export function App() {
  const [, setS] = createSignal(0);
  function h() { return run({ go: setS }); }
  return <p>{h()}</p>;
}
