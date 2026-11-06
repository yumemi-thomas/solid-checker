import { createMemo, type Accessor } from "solid-js";
function C(props: { get: Accessor<number> }) {
  const m = createMemo(() => props.get());
  return <time>{m()}</time>;
}
export function Prop() {
  const src = createMemo(async () => { await 0; return 1; });
  return <C get={src} />;
}
