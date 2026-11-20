import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const st = createSignal(1);
  const g = st[0];
  return <div>{g()}</div>;
}
