import { onCleanup, createRoot } from "solid-js";
function handlers(cb: () => void) {
  const run = () => createRoot(() => cb());
  return { onClick: run };
}
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}
