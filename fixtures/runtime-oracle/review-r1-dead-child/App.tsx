import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: {children: () => JSX.Element}) {
  if (false) props.children();
  return <div>{props.children()}</div>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const v = n(); return <span>{v}</span>; }}</Kids>;
}
