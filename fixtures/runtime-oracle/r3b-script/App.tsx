import { createSignal } from "solid-js";

export function App() {
  const [count] = createSignal(0);
  const h = relay(() => console.log(count()));
  fireNow(h);
  return <button onClick={h.onClick}>x</button>;
}
