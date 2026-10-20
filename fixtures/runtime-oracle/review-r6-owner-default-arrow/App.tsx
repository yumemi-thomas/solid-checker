import { createRoot, onCleanup } from "solid-js";
function handlers(cb: () => void) {
  return { onClick: () => {
    const later = (value = cb()) => value;
    createRoot(() => { later(); });
  }};
}
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}
