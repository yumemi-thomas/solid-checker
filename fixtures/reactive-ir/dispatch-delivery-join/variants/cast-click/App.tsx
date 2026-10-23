import { onCleanup } from "solid-js";

// `(b as HTMLButtonElement).click()` is an argumentless call: it resolves to
// no declaration at the call, yet it dispatches `click` synchronously.
export function CastClick() {
  const a = (<div onScroll={() => undefined} />) as HTMLDivElement;
  a.dispatchEvent(new Event("scroll"));
  const b = (<button onClick={() => { onCleanup(() => undefined); }} />) as HTMLButtonElement;
  (b as HTMLButtonElement).click();
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
