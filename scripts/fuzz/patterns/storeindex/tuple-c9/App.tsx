import { createSignal, createStore, createMemo, createProjection } from "solid-js";
export function App() {
  const st = createStore({ v: 1 });
  const read = () => st[0].v;
  read();
  return <div />;
}
