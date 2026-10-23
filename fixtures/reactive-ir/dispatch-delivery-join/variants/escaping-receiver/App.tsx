import { onCleanup } from "solid-js";

// The element is also rendered: any reference that is not a member read may
// hand it to code that gives it listeners the attributes do not show.
export function Escaping() {
  const el = (<div onScroll={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  return <button onClick={() => { onCleanup(() => undefined); }}>{el}</button>;
}
