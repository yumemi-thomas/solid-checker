import { createSignal, createMemo } from "solid-js";
export function Derived() {
  const a = createMemo(async () => { await 0; return 1; });
  const [s, setS] = createSignal(() => a() + 1);
  const read = () => s();
  return <time>{read()}</time>;
}
