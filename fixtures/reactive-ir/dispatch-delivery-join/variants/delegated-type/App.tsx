import { onCleanup } from "solid-js";

// `click` is in the delegated set: its handler is stored on the node and run
// from a container listener, so the element's own listeners do not bound it.
export function Delegated() {
  const el = (<div onClick={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  el.dispatchEvent(new Event("click"));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
