import { createSignal } from "solid-js";

export function App() {
  const [count] = createSignal(1);
  const doubled = count() * 2;
  return <p>{doubled}</p>;
}
