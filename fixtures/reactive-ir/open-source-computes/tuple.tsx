import { createMemo, type Accessor } from "solid-js";
function useX(a: Accessor<number>) {
  const m = createMemo(() => a());
  return [m] as const;
}
export function Tuple() {
  const src = createMemo(async () => { await 0; return 1; });
  const [m] = useX(src);
  return <time>{m()}</time>;
}
