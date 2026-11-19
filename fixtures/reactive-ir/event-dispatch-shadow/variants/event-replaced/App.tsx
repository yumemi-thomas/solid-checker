import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1125 (G1): `globalThis.event` replaced by a data property no delivery sets again: the light-tree delivery from the apply runs the
// handler with no event, inside the apply's strict-read window (Chrome rc.21: STRICT_READ_UNTRACKED). The read stays uncertifiable.
globalThis.event = undefined;

export function EventReplaced() {
  const box = (<div onScroll={() => { count(); }} />) as HTMLDivElement;
  createEffect(
    () => 1,
    () => {
      box.dispatchEvent(new Event("scroll"));
    },
  );
  return <div />;
}
