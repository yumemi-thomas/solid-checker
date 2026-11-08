import { createSignal, createMemo, Loading } from "solid-js";
function useX() {
  const [s] = createSignal(1);
  const a = createMemo(async () => { await 0; return 1; });
  return [s, a] as const;
}
export function SyncOfMixed() {
  const [x, y] = useX();
  return <time>{x()}<Loading fallback="w">{y()}</Loading></time>;
}
