import { createSignal } from "solid-js";
import { view } from "fz-view";
const [count] = createSignal(1);
function Child({ v }: { v: { readonly search: number } }) {
  const s = v.search;
  return <i>{s}</i>;
}
export function App() {
  const v = view(count);
  return <Child v={v} />;
}
