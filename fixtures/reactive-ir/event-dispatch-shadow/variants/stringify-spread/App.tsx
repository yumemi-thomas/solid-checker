import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1125 (Fable round 2, N1): `JSON.stringify` handed its replacer by a spread takes Solid's stored delegated handler (`_$$click`) off the element and the apply calls it with
// no event, inside the apply's strict-read window (the same run as `Object.values`, which warns STRICT_READ_UNTRACKED in Chrome
// rc.21). The read stays uncertifiable.
export function StoredHandler() {
  const box = (<div onClick={() => { count(); }} />) as HTMLDivElement;
  createEffect(
    () => 1,
    () => {
      JSON.stringify(...([box, (_key: string, value: unknown) => { if (typeof value === "function") (value as (event: Event) => void)(new Event("click")); return value; }] as [object, (key: string, value: unknown) => unknown]));
    },
  );
  return <div />;
}
