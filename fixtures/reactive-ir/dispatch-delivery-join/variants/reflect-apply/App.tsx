import { onCleanup } from "solid-js";

// The dispatch is not a call of `dispatchEvent`: the function is handed to
// `Reflect.apply`.
export function ReflectApply() {
  const a = (<div onScroll={() => undefined} />) as HTMLDivElement;
  a.dispatchEvent(new Event("scroll"));
  const b = (<div onScroll={() => { onCleanup(() => undefined); }} />) as HTMLDivElement;
  Reflect.apply(EventTarget.prototype.dispatchEvent, b, [new Event("scroll")]);
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
