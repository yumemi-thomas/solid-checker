import { createSignal, createMemo } from "solid-js";
function useX() {
  const [s] = createSignal(1);
  const a = createMemo(async () => { await 0; return 1; });
  const t: (() => number)[] = [s];
  t[0] = a;
  return t;
}
export function HeldThenWritten() {
  const [m] = useX();
  return <time>{m()}</time>;
}
