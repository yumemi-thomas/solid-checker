import { createSignal, omit } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: { children: () => JSX.Element }) {
  const saved = props.children;
  const view = omit(props);
  Reflect.set(view, "children", () => <span>replacement</span>);
  props.children();
  return <div>{saved()}</div>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const value = n(); return <span>{value}</span>; }}</Kids>;
}
