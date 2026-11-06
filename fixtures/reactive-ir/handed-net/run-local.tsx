import { createMemo, createSignal } from "solid-js";
export function App() {
  const m = createMemo(() => {
    const run = (o: { go: (v: number) => void }) => { o.go(1); return 1; }; const [, w] = createSignal(0); run({ go: w });
    return 1;
  });
  return <p>{m()}</p>;
}
