import { createSignal, Repeat } from "solid-js";
export function App() {
  const [n] = createSignal(5);
  return <Repeat count={1}>{(i) => { const v = n(); return <p id="mk5">{v + i}</p>; }}</Repeat>;
}
