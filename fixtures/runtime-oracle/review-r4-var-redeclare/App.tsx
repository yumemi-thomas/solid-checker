import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
type Props = { children: () => JSX.Element };
function Kids(props: Props) {
  var props: Props = { children: () => <span>replacement</span> };
  return props.children();
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const value = n(); return <span>{value}</span>; }}</Kids>;
}
