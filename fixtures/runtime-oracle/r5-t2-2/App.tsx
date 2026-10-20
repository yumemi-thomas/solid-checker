import { createSignal } from "solid-js";
import { keepA, keepB } from "./lib";

export function App() {
  const [count] = createSignal(0);
  const a = keepA(() => console.log("a", count()));
  const b = keepB(() => console.log("b", count()));
  return <button onClick={() => (a(), b())}>x</button>;
}
