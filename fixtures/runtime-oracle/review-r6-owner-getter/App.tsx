import { createRoot, onCleanup } from "solid-js";
function handlers(cb: () => void) {
  return { onClick: () => {
    const later = { get value() { cb(); return 1; } };
    createRoot(() => { void later.value; });
  }};
}
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}
