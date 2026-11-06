import { createMemo, type Accessor } from "solid-js";
function C(props: { get: Accessor<number> }) {
  const m = createMemo(() => props.get());
  let useIt = m;
  useIt = m;
  return <time>{useIt()}</time>;
}
export function Let() {
  const src = createMemo(async () => { await 0; return 1; });
  return <C get={src} />;
}
