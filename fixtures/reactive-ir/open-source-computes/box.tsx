import { createMemo, type Accessor } from "solid-js";
function C(props: { get: Accessor<number> }) {
  const m = createMemo(() => props.get());
  const box = { useIt: m };
  return <time>{box.useIt()}</time>;
}
export function Box() {
  const src = createMemo(async () => { await 0; return 1; });
  return <C get={src} />;
}
