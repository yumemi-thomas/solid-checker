import { createMemo, type Accessor } from "solid-js";
function useX(a: Accessor<number>) {
  const m = createMemo(() => a());
  return () => m();
}
export function Closure() {
  const src = createMemo(async () => { await 0; return 1; });
  const read = useX(src);
  return <time>{read()}</time>;
}
