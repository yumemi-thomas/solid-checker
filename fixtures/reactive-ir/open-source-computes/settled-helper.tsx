import { createMemo, createSignal } from "solid-js";
function useX() {
  const [src] = createSignal(1);
  const h = () => src() + 1;
  const m = createMemo(() => h());
  return [m] as const;
}
export function SettledHelper() {
  const [m] = useX();
  return <time>{m()}</time>;
}
