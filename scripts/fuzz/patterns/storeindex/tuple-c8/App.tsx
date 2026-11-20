import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const st = createStore({ v: 1 });
  const st2 = st;
  void st2[0].v;
  return <div />;
}
