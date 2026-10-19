import { createSignal, Switch, Match } from "solid-js";
export function App() {
  const [on] = createSignal(true);
  const [n] = createSignal(2);
  return <Switch><Match when={on()}>{() => { const v = n(); return <p id="mk2">{v}</p>; }}</Match></Switch>;
}
