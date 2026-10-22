import { onCleanup } from "solid-js";

// An object handler is a capture listener when `capture` is true (`addEvent`
// passes the handler object as the listener options), so it can run for an
// event dispatched on a descendant: it is not a proof site. The supported
// dispatch on `a` proves nothing about it, and the unrelated function handler
// beside it keeps its proof.
export function ObjectHandler() {
  const a = (<div onScroll={() => undefined} />) as HTMLDivElement;
  a.dispatchEvent(new Event("scroll"));
  return (
    <section onScroll={{ handleEvent() { onCleanup(() => undefined); }, capture: true }}>
      <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>
    </section>
  );
}
