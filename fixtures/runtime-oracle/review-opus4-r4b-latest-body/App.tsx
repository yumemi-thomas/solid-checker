import { createSignal, latest } from "solid-js";
export function App() {
  const [n] = createSignal(1);
  const v = latest(() => n());
  return <p id="mk6">{v}</p>;
}
