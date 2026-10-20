import { createSignal, Show, Switch, Match, createMemo, createEffect, onCleanup, action, untrack } from "solid-js";
import type { JSX } from "@solidjs/web";

export function App() {
  const [on] = createSignal(true);
  const [k, setK] = createSignal(0);
  return <div><Switch><Match when={on()}>{(...a: any[]) => <button id="b" onClick={() => setK(1)}>{a.length}</button>}</Match></Switch><p id="k">{k()}</p></div>;
}
