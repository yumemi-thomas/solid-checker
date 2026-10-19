import { getOwner, onCleanup } from "solid-js";
function handlers(cb: () => void) { return { onScroll: () => cb() }; }
export function App() {
  let dispatched = 0;
  const h = handlers(() => {
    dispatched++;
    if (!getOwner()) throw new Error("expected synchronous owner");
    onCleanup(() => undefined);
  });
  const el = <div {...h} /> as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  if (dispatched !== 1) throw new Error("handler was not executed exactly once");
  return <p id="oracle-dispatch">dispatched synchronously</p>;
}
