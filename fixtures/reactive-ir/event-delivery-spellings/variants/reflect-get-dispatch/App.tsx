import { createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F1): `Reflect.get(box, "dispatchEvent")` is the dispatcher the dispatch census cannot see as written: the handler runs on the body's stack.
export function Page() {
  const box = (<div onScroll={() => { count(); }} />) as HTMLDivElement;
  (Reflect.get(box, "dispatchEvent") as any).call(box, new Event("scroll"));
  return box;
}
