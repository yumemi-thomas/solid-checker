import { onCleanup } from "solid-js";
function handlers(cb: () => void) { return { onClick: [() => cb(), 1] as [() => void, number] }; }
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>tuple dispatch</button>;
}
