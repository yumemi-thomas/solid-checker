import { createSignal, createMemo } from "solid-js";
function useX() {
  const [s] = createSignal(1);
  const a = createMemo(async () => { await 0; return 1; });
  return [a, s] as const;
}
export function Hole() {
  const [, m] = useX();
  return <time>{m()}</time>;
}
