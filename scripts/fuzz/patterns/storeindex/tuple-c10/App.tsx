import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const st = createStore({ v: 1 });
  const k = 0 as number;
  void st[k];
  return <div>{st[0].v}</div>;
}
