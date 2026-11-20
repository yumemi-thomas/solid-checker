import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const st = createStore({ v: 1 });
  const [o] = st;
  void o.v;
  return <div />;
}
