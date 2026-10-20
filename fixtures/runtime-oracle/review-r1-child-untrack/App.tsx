import { createSignal, untrack } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: {children: () => JSX.Element}) {
  return untrack(() => props.children());
}
export function App() {
  const [n] = createSignal(0);
  return <Kids>{() => { const v = n(); return <span>{v}</span>; }}</Kids>;
}
