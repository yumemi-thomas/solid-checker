import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: { children: () => JSX.Element }) {
  function later(value = props.children()) { return value; }
  return <button onClick={() => { later(); }}>click</button>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const value = n(); return <span>{value}</span>; }}</Kids>;
}
