import { onCleanup } from "solid-js";
function handlers(cb: () => void) { return { onScroll: () => cb() }; }
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <div {...h}>normal dispatch</div>;
}
