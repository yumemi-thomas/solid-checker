import { onCleanup } from "solid-js";

// `checkValidity()` fires `invalid` synchronously (ADR 0329): a dispatch
// beside the supported one that no site models.
export function CheckValidity() {
  const a = (<div onScroll={() => undefined} />) as HTMLDivElement;
  a.dispatchEvent(new Event("scroll"));
  const i = (<input required onInvalid={() => { onCleanup(() => undefined); }} />) as HTMLInputElement;
  i.checkValidity();
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
