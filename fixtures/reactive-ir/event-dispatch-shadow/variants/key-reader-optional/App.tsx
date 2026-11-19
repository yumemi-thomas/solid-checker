import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1125 (Fable round 2, N1): `Object?.["values"]`, the reader by an optional computed key, takes Solid's stored delegated handler (`_$$click`) off the element and the apply calls it with
// no event, inside the apply's strict-read window (the same run as `Object.values`, which warns STRICT_READ_UNTRACKED in Chrome
// rc.21). The read stays uncertifiable.
export function StoredHandler() {
  const box = (<div onClick={() => { count(); }} />) as HTMLDivElement;
  createEffect(
    () => 1,
    () => {
      for (const value of (Object as typeof Object)?.["values"](box as object)) if (typeof value === "function") (value as (event: Event) => void)(new Event("click"));
    },
  );
  return <div />;
}
