import { onCleanup } from "solid-js";
export function App() {
  const g1 = () => {
    onCleanup(() => {});
  };
  setTimeout(() => { g1(); }, 0);
  return <i>app</i>;
}
