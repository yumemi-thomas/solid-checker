import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const st = createStore({ v: 1 });
  void st[0].v;
  return <div />;
}
