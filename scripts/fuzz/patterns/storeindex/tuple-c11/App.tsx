import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const [s] = createStore({ v: 1 });
  void s.v;
  return <div />;
}
