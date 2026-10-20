import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: { render: () => JSX.Element }) {
  for (props.render of [() => <span>replacement</span>]) {}
  return props.render();
}
export function App() {
  const [n] = createSignal(0);
  return <Kids render={() => { const value = n(); return <span>{value}</span>; }} />;
}
