import { onCleanup } from "solid-js";

// The receiver is a parameter: no element is named.
function fire(target: EventTarget) {
  target.dispatchEvent(new Event("scroll"));
}

export function UnknownReceiver() {
  const el = (<div onScroll={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  fire(el);
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
