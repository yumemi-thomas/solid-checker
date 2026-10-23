import { onCleanup } from "solid-js";

// `window` here is a parameter, not the global: its dispatch may reach anything.
function fire(window: EventTarget) {
  window.dispatchEvent(new CustomEvent("app:ping"));
}

export function ShadowedWindow() {
  const el = (<div onScroll={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  fire(el);
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
