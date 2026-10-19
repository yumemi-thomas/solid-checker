import { createSignal, merge } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: { children: () => JSX.Element }) {
  const saved = props.children;
  delete (merge(props) as { children?: () => JSX.Element }).children;
  Object.setPrototypeOf(merge(props), { children: () => <span>replacement</span> });
  props.children();
  return <div>{saved()}</div>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const value = n(); return <span>{value}</span>; }}</Kids>;
}
