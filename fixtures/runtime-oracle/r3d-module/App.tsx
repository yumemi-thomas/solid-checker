import { createSignal } from "solid-js";
import { relay, fireNow } from "./lib";

export function App() {
  const [count] = createSignal(0);
  const h = relay(() => console.log(count()));
  fireNow(h.now);
  return <button onClick={h.onClick}>x</button>;
}
