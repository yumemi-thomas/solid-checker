import { createSignal, createMemo } from "solid-js";
function useX() {
  const [s] = createSignal(1);
  const a = createMemo(async () => { await 0; return 1; });
  return [s, a] as const;
}
export function AsyncOfMixed() {
  const [x, y] = useX();
  return <time>{x()}{y()}</time>;
}
