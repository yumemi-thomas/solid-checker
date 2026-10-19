import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
type Props = { children: () => JSX.Element; replace: () => void };
function Kids(props: Props) {
  const saved = props.children;
  props.replace();
  props.children();
  return <div>{saved()}</div>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids replace={function(this: Props) { this.children = () => <span>replacement</span>; }}>
    {() => { const value = n(); return <span>{value}</span>; }}
  </Kids>;
}
