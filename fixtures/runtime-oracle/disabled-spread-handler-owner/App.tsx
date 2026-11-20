import { createEffect, createStore } from "solid-js";

// ADR 1141: with a spread, the compiler hands every attribute and spread to rc.21's `spread` as one array of sources, whose
// `assignProp` delegates a listener key (`onClick`, `onKeyDown`) to the dispatcher that passes over a node whose `disabled` is true.
// A control disabled from mount whose spreads have proven keys, none of them `disabled`, never runs such a handler: its SC4001 row
// is an obligation (`handler-on-disabled-control`).
export function SpreadDisabled() {
  const [s1] = createStore({ v: 0 });
  const rest = { title: "t" };
  return (
    <div>
      <button disabled {...rest} onClick={() => { createEffect(() => s1.v, () => {}); }}>rest</button>
      <button disabled {...{ onClick: () => { createEffect(() => s1.v, () => {}); } }}>literal</button>
      <input disabled {...{ title: "t" }} onKeyDown={() => { createEffect(() => s1.v, () => {}); }} />
    </div>
  );
}

// Violations still: each of these handlers runs.
export function SpreadStillRuns() {
  const [s1] = createStore({ v: 0 });
  const enable: { disabled?: boolean } = { disabled: false };
  const loose = JSON.parse("{\"disabled\":false}") as { title?: string };
  return (
    <div>
      {/* The spread writes `disabled` back to false. */}
      <button disabled {...enable} onClick={() => { createEffect(() => s1.v, () => {}); }}>enable</button>
      {/* The spread's keys are not proven (its value is no literal). */}
      <button disabled {...loose} onClick={() => { createEffect(() => s1.v, () => {}); }}>loose</button>
      {/* `wheel` is not delegated: a listener the browser calls on a disabled control. */}
      <button disabled {...{ onWheel: () => { createEffect(() => s1.v, () => {}); } }}>wheel</button>
      {/* The driven twin. */}
      <button {...{ onClick: () => { createEffect(() => s1.v, () => {}); } }}>driven</button>
    </div>
  );
}
