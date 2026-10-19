import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: { children: () => JSX.Element }) {
  return props.children();
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{(() => { const value = n(); return <span>{value}</span>; }) satisfies (() => JSX.Element)}</Kids>;
}
