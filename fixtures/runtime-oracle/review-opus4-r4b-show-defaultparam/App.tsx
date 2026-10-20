import { createSignal, Show } from "solid-js";
export function App() {
  const [on] = createSignal(true);
  const [n] = createSignal(4);
  return <Show when={on()}>{(x = () => true) => { const v = n(); return <p id="mk4">{v}{x() ? "" : "!"}</p>; }}</Show>;
}
