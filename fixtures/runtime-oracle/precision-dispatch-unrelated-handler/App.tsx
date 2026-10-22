import { onCleanup } from "solid-js";
export function App() {
  const probe = (<div onScroll={() => undefined} />) as HTMLDivElement;
  probe.dispatchEvent(new Event("scroll"));
  return <button id="oracle-unrelated" onClick={() => { onCleanup(() => undefined); }}>unrelated handler</button>;
}
