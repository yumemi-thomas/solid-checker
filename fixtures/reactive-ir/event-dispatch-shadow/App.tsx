import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// A read in an element's `onScroll` (a listener on the element itself) that an effect's apply delivers by `dispatchEvent`, in the
// light tree: the delivery sets `window.event`, so the apply's strict-read window (keyed on its queue's event) is closed for the read
// (ADR 0639). Proved while no listener of the program may sit in a shadow tree.
export function Light() {
  const box = (<div onScroll={() => { count(); }} />) as HTMLDivElement;
  createEffect(
    () => 1,
    () => {
      box.dispatchEvent(new Event("scroll"));
    },
  );
  return <div />;
}
