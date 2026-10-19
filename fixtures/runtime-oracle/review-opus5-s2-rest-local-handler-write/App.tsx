import { createSignal, Show, Switch, Match, createMemo, createEffect, onCleanup, action, untrack } from "solid-js";
import type { JSX } from "@solidjs/web";

export function App() {
  const [on] = createSignal(true);
  const [k, setK] = createSignal(0);
  const [n] = createSignal(4);
  return <div><Show when={on()}>{(...a: any[]) => { const h = () => setK(1); return <button id="b" onClick={h}>{a.length}</button>; }}</Show><p id="k">{k()}</p></div>;
}
