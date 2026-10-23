import { onCleanup } from "solid-js";

class Event {
  constructor(readonly type: string) {}
}

// `Event` here is a project class, not the `lib.dom` global; the dispatch
// argument does not even type-check as a DOM event, so the call is cast.
export function Shadowed() {
  const el = (<div onScroll={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  el.dispatchEvent(new Event("scroll") as unknown as globalThis.Event);
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
