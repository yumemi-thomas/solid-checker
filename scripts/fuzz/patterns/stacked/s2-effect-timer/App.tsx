import { createEffect } from "solid-js";
export function App() {
  const g1 = () => {
    createEffect(() => 1, () => {});
  };
  setTimeout(() => { g1(); }, 0);
  return <i>app</i>;
}
