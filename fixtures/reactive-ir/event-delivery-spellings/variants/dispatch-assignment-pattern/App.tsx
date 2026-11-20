import { createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F3): a destructuring assignment takes `dispatchEvent` out of the element; the pattern's key is a name the census cannot follow.
export function Page() {
  const box = (<div onScroll={() => { count(); }} />) as HTMLDivElement;
  let dispatch: any; ({ dispatchEvent: dispatch } = box as any); dispatch.call(box, new Event("scroll"));
  return box;
}
