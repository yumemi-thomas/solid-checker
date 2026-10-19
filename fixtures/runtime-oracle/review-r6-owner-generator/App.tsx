import { createRoot, onCleanup } from "solid-js";
function handlers(cb: () => void) {
  return { onClick: () => {
    function* later() { cb(); yield 1; }
    const iterator = later();
    createRoot(() => { iterator.next(); });
  }};
}
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}
