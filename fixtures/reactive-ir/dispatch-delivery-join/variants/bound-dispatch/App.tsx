import { onCleanup } from "solid-js";

// `b.dispatchEvent` is a value here, not a call: `fire.call(b, …)` dispatches
// wherever the function is called.
export function BoundDispatch() {
  const a = (<div onScroll={() => undefined} />) as HTMLDivElement;
  a.dispatchEvent(new Event("scroll"));
  const b = (<div onScroll={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  const fire = b.dispatchEvent;
  fire.call(b, new Event("scroll"));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
