import { createMemo, type Accessor } from "solid-js";
function C(props: { get: Accessor<number> }) {
  const m = createMemo(() => props.get());
  const alias = m;
  return <time>{alias()}</time>;
}
export function Alias() {
  const src = createMemo(async () => { await 0; return 1; });
  return <C get={src} />;
}
