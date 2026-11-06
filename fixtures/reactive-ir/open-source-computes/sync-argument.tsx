import { createMemo, createSignal, type Accessor } from "solid-js";
function useX(a: Accessor<number>) {
  const m = createMemo(() => a());
  return [m] as const;
}
export function SyncArgument() {
  const [src] = createSignal(1);
  const [m] = useX(src);
  return <time>{m()}</time>;
}
