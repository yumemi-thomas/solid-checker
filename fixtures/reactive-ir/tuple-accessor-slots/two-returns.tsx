import { createSignal, createMemo } from "solid-js";
function useX(c: boolean) {
  const [s] = createSignal(1);
  const a = createMemo(async () => { await 0; return 1; });
  if (c) return [s] as const;
  return [a] as const;
}
export function TwoReturns() {
  const [m] = useX(Math.random() > 2);
  return <time>{m()}</time>;
}
