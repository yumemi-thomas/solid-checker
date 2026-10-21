import { getOwner, onCleanup } from "solid-js";
function Child(props: { children: () => void }) {
  const el = <div onScroll={() => props.children()} /> as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  return <p id="oracle-dispatch">dispatched child synchronously</p>;
}
export function App() {
  let dispatched = 0;
  return <Child>{() => {
    dispatched++;
    if (!getOwner()) throw new Error("expected synchronous owner");
    onCleanup(() => undefined);
  }}</Child>;
}
