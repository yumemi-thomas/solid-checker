import { createSignal, Show, Switch, Match, createMemo, createEffect, onCleanup, onSettled, action, untrack } from "solid-js";
import type { JSX } from "@solidjs/web";

export function App() {
  const [on] = createSignal(true);
  const [k, setK] = createSignal(0);
  const [n] = createSignal(4);
  return <div><Show when={on()}>{(w) => { setTimeout(() => setK(2), 0); return <p>{w() ? 1 : 0}</p>; }}</Show><p id="k">{k()}</p></div>;
}
