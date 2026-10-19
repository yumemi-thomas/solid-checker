import { onCleanup } from "solid-js";
function handlers(cb: () => void) {
  return { onClick: () => {
    function later(value = cb()) { return value; }
    later();
  }};
}
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}
