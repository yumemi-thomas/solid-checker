import { createSignal, merge } from "solid-js";
import type { JSX } from "@solidjs/web";
import { replace } from "./mutate";
function Kids(props: { children: () => JSX.Element }) {
  const saved = props.children;
  replace(merge(props));
  props.children();
  return <div>{saved()}</div>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const value = n(); return <span>{value}</span>; }}</Kids>;
}
