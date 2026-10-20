import { createSignal, Show } from "solid-js";
export function App() {
  const [on] = createSignal(true);
  const [n] = createSignal(3);
  return <Show when={on()}>{(x) => { const v = n(); return <p id="mk3">{v}{x() ? "" : "!"}</p>; }}</Show>;
}
