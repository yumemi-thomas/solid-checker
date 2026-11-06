import { createMemo } from "solid-js";
function C(props: { label: (n: number) => string }) {
  const m = createMemo(() => props.label(1));
  return <time>{m()}</time>;
}
export function QuietCallback() {
  return <C label={(n) => "x" + n} />;
}
