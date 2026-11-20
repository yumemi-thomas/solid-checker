import { createEffect, createSignal, createStore } from "solid-js";

// ADR 1134: handlers the delegated dispatcher passes over (a form control disabled from mount): Chrome reports nothing, and the
// checker's SC4001 rows there are obligations.
export function MountDisabled() {
  const [s1] = createStore({ v: 0 });
  return (
    <div>
      <button disabled onClick={() => { createEffect(() => s1.v, () => {}); }}>go</button>
      <button disabled={true} onClick={() => { createEffect(() => s1.v, () => {}); }}>literal</button>
      <input disabled onKeyDown={() => { createEffect(() => s1.v, () => {}); }} />
      <button disabled onPointerDown={() => { createEffect(() => s1.v, () => {}); }}>pointer</button>
    </div>
  );
}

// Handlers that run although the control is disabled, or is not: Chrome reports NO_OWNER_EFFECT at each, covered by SC4001.
export function StillRuns() {
  const [s1] = createStore({ v: 0 });
  const [off] = createSignal(false);
  return (
    <div>
      <button disabled onWheel={() => { createEffect(() => s1.v, () => {}); }}>wheel</button>
      <button disabled={off()} onClick={() => { createEffect(() => s1.v, () => {}); }}>dynamic</button>
      <button onClick={() => { createEffect(() => s1.v, () => {}); }}>driven</button>
    </div>
  );
}
