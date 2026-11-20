import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const st = createStore({ v: 1 });
  const o = st[0];
  return <div>{o.v}</div>;
}
