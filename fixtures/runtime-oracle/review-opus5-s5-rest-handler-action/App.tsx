import { createSignal, Show, Switch, Match, createMemo, createEffect, onCleanup, action, untrack } from "solid-js";
import type { JSX } from "@solidjs/web";

export function App() {
  const [on] = createSignal(true);
  const [k, setK] = createSignal(0);
  const [n] = createSignal(4);
  const save = action(function* (v: number) { setK(v); });
  return <div><Show when={on()}>{(...a: any[]) => <button id="b" onClick={() => save(1)}>{a.length}</button>}</Show><p id="k">{k()}</p></div>;
}
