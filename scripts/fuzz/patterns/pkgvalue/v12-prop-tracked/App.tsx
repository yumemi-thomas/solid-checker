import { createSignal } from "solid-js";
import { view } from "fz-view";
const [count] = createSignal(1);
function Child(props: { v: { readonly search: number } }) {
  return <i>{props.v.search}</i>;
}
export function App() {
  const v = view(count);
  return <Child v={v} />;
}
