import { createSignal, createMemo, createEffect } from "solid-js";
const [sig, setSig] = createSignal(0);
function run(o: { go: (v: number) => void }) { o.go(1); }
export function App() {
  createMemo(() => { run({ go: () => {} }); return 0; });
  return <i>x</i>;
}
