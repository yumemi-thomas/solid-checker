import { createMemo, createSignal } from "solid-js";
function useX() {
  const [src] = createSignal(1);
  const m = createMemo(() => src() + 1);
  return [m] as const;
}
export function Settled() {
  const [m] = useX();
  return <time>{m()}</time>;
}
