import { onCleanup } from "solid-js";

// A bubbling event reaches handlers on ancestors the program does not show:
// every handler, the unrelated one included, may run on the dispatcher's stack.
export function Bubbles() {
  const el = (<div onPing={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  el.dispatchEvent(new CustomEvent("ping", { bubbles: true }));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
