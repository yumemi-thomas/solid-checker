import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
let Kids = (props: { children: () => JSX.Element }): JSX.Element => {
  return props.children();
};
function Later(props: { children: () => JSX.Element }) {
  return <button onClick={() => { props.children(); }}>click</button>;
}
Kids = Later;
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const value = n(); return <span>{value}</span>; }}</Kids>;
}
