import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: {children: () => JSX.Element}) {
  const saved = props.children;
  const alias = props;
  alias.children = () => <span>replacement</span>;
  const replacement = props.children();
  return <div id="oracle-replacement">{replacement}{saved()}</div>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const v = n(); return <span>{v}</span>; }}</Kids>;
}
