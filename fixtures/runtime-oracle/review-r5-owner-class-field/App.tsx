import { createRoot, onCleanup } from "solid-js";
function handlers(cb: () => void) {
  return { onClick: () => {
    class Later { value = cb(); }
    createRoot(() => { new Later(); });
  }};
}
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}
