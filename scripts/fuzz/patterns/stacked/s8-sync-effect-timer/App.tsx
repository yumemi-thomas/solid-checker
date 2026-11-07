import { createEffect } from "solid-js";
export function App() {
  const g1 = () => {
    createEffect(() => 1, () => {}, { sync: true });
  };
  setTimeout(() => { g1(); }, 0);
  return <i>app</i>;
}
