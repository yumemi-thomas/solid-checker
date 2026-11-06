import { createMemo } from "solid-js";
function C(props: { items: number[]; name: string }) {
  const m = createMemo(() => props.items.map((i) => i + 1).join(props.name.trim()));
  return <time>{m()}</time>;
}
export function Standard() {
  return <C items={[1]} name="a" />;
}
