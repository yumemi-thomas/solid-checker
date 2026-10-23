import { onCleanup } from "solid-js";

// A `ref` binding is not an exact receiver here: the element is assigned
// after construction and the binding is written by the ref application.
export function RefReceiver() {
  let el!: HTMLDivElement;
  const view = <div ref={el} onScroll={() => { onCleanup(() => undefined); }} />;
  el.dispatchEvent(new Event("scroll"));
  return <button onClick={() => { onCleanup(() => undefined); }}>{view}</button>;
}
