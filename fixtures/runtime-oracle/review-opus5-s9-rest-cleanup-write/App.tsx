import { createSignal, Show, Switch, Match, createMemo, createEffect, onCleanup, action, untrack } from "solid-js";
import type { JSX } from "@solidjs/web";

export function App() {
  const [on] = createSignal(true);
  const [k, setK] = createSignal(0);
  const [n] = createSignal(4);
  return <div><Show when={on()}>{(...a: any[]) => { onCleanup(() => setK(9)); return <p>{a.length}</p>; }}</Show><p id="k">{k()}</p></div>;
}
