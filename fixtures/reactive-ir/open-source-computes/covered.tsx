import { createMemo, Loading, type Accessor } from "solid-js";
function C(props: { get: Accessor<number> }) {
  const m = createMemo(() => props.get());
  return <time>{m()}</time>;
}
export function Covered() {
  const src = createMemo(async () => { await 0; return 1; });
  return <Loading fallback="w"><C get={src} /></Loading>;
}
