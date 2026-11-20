import { createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F3): `box["dispatch" + "Event"].call(…)` dispatches under a key the program builds; the census reads only keys written as names or literals.
export function Page() {
  const box = (<div onScroll={() => { count(); }} />) as HTMLDivElement;
  (box as any)["dispatch" + "Event"].call(box, new Event("scroll"));
  return box;
}
