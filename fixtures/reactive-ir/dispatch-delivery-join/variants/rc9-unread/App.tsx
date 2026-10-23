import { onCleanup } from "solid-js";

// The root's `Targeted` on another web release: the delegated set is the
// release's answer (ADR 0361).
export function Targeted() {
  const el = (<div onScroll={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  el.dispatchEvent(new Event("scroll"));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
