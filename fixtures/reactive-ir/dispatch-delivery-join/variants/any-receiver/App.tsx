import { onCleanup } from "solid-js";

// A supported dispatch on `a`, and one on an `any` receiver the checker cannot
// see through: the second may run any handler.
export function AnyReceiver() {
  const a = (<div onScroll={() => undefined} />) as HTMLDivElement;
  a.dispatchEvent(new Event("scroll"));
  const b = (<div onScroll={() => { onCleanup(() => undefined); }} />) as any;
  b.dispatchEvent(new Event("scroll"));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
