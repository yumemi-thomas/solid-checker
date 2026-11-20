import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const st = createStore([1, 2]);
  const o = st[0];
  return <div>{o[0]}</div>;
}
