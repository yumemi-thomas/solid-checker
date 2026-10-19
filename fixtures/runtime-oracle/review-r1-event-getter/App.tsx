import { onCleanup } from "solid-js";
function handlers(cb: () => void) {
  return { get onClick() { cb(); return () => undefined; } };
}
export function App() {
  const h = handlers(() => {onCleanup(() => undefined);});
  return <button {...h}>getter</button>;
}
