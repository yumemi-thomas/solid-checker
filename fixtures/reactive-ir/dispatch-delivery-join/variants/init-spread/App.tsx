import { onCleanup } from "solid-js";

const init: CustomEventInit = { bubbles: true };

// The init is not a static object literal: it may name `bubbles: true`.
export function InitSpread() {
  const el = (<div onPing={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  el.dispatchEvent(new CustomEvent("ping", { ...init }));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
