import { createSignal } from "solid-js";
export function App() {
  const [label] = createSignal("x");
  return <div ref={el => { el.title = label(); }}>read</div>;
}
