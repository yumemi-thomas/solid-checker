import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: { render: () => JSX.Element }) {
  const saved = props.render;
  Reflect.set(arguments[0], "render", () => <span>replacement</span>);
  props.render();
  return <div>{saved()}</div>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids render={() => { const value = n(); return <span>{value}</span>; }} />;
}
