import { createSignal } from "solid-js";
import { view } from "fz-view";
const [count] = createSignal(1);
function show(l: { readonly search: number }) {
  return l.search;
}
export function App() {
  const s = show(view(count));
  return <i>{s}</i>;
}
