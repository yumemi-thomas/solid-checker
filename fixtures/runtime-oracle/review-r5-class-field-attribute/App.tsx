import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
function Kids(props: { render: () => JSX.Element }) {
  class Later { value = props.render(); }
  return <button onClick={() => { new Later(); }}>click</button>;
}
export function App() {
  const [n] = createSignal(0);
  return <Kids render={() => { const value = n(); return <span>{value}</span>; }} />;
}
