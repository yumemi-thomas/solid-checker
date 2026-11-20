import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const st = createStore({ v: 1 });
  const set = st[1];
  set((d) => { d.v = 2; });
  return <div>{st[0].v}</div>;
}
