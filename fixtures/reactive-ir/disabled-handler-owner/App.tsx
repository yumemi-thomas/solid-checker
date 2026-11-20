import { createEffect, createSignal, createStore } from "solid-js";

// ADR 1134: Solid's delegated dispatcher passes over a node whose `disabled` is true, so a handler on a control disabled from
// mount, which nothing enables, never runs. Its SC4001 missing-owner row is an obligation (`handler-on-disabled-control`).
export function MountDisabled() {
  const [s1] = createStore({ v: 0 });
  return (
    <div>
      <button disabled onClick={() => {
        { const f = () => s1.v; createEffect(f, () => {}); f(); };
      }}>go</button>
      <button disabled={true} onClick={() => { createEffect(() => s1.v, () => {}); }}>literal</button>
      <input disabled onKeyDown={() => { createEffect(() => s1.v, () => {}); }} />
      <button disabled onPointerDown={() => { createEffect(() => s1.v, () => {}); }}>pointer</button>
    </div>
  );
}

// Violations still: each of these handlers runs.
export function StillRuns() {
  const [s1] = createStore({ v: 0 });
  const [off] = createSignal(true);
  return (
    <div>
      {/* `disabled` is no property of a div: the dispatcher calls its handler. */}
      <div disabled onClick={() => { createEffect(() => s1.v, () => {}); }}>div</div>
      {/* `wheel` is not delegated: a listener the browser calls on a disabled control. */}
      <button disabled onWheel={() => { createEffect(() => s1.v, () => {}); }}>wheel</button>
      {/* A `disabled` that follows a signal may become false. */}
      <button disabled={off()} onClick={() => { createEffect(() => s1.v, () => {}); }}>dynamic</button>
      {/* The driven twin. */}
      <button onClick={() => { createEffect(() => s1.v, () => {}); }}>driven</button>
    </div>
  );
}
