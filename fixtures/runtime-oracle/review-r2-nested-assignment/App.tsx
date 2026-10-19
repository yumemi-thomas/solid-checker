import { createSignal, merge } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: { children: () => JSX.Element }) {
  const saved = props.children;
  const view = merge(props);
  function replace() { view.children = () => <span>replacement</span>; }
  replace();
  const replacement = props.children();
  return <div id="oracle-replacement">{replacement}{saved()}</div>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const value = n(); return <span>{value}</span>; }}</Kids>;
}
