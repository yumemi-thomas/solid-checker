import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
type Props = { children: () => JSX.Element; replace: new () => object };
function Kids(props: Props) {
  new props.replace();
  return props.children();
}
export function App() {
  const [n] = createSignal(0);
  return <Kids replace={function(this: Props) { this.children = () => <span>replacement</span>; } as unknown as new () => object}>
    {() => { const value = n(); return <span>{value}</span>; }}
  </Kids>;
}
