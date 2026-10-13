import { createSignal } from "solid-js";

export function App() {
  const [value] = createSignal(0);
  const snapshot = value(); // strict-read violation under the premise
  return <div>{snapshot}{value()}</div>; // tracked twin remains clean
}
