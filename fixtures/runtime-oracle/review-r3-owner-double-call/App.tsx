import { onCleanup } from "solid-js";
function handlers(cb: () => void) {
  const run = () => cb();
  run();
  return { onClick: run };
}
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}
