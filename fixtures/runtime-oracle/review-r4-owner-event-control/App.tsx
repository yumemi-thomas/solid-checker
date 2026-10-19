import { onCleanup } from "solid-js";
function handlers(cb: () => void) {
  return { onClick: () => cb() };
}
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}
