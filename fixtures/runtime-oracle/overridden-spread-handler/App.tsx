import { createRenderEffect, createStore } from "solid-js";

// ADR 1140: rc.21's spread skips a key a later source of the element has, so a handler a later attribute, a later spread or a
// later `undefined` overrides is never attached: Chrome reports nothing there, and the checker's SC4001 rows are obligations.
export function Overridden() {
  const [s1] = createStore({ v: 0 });
  const later = () => {};
  const extra: { onClick?: () => void } = { onClick: later };
  return (
    <div>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} onClick={later}>attribute</button>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} {...{ onClick: later }}>spread</button>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} onClick={undefined}>undefined</button>
      <button onClick={() => { createRenderEffect(() => s1.v, () => {}); }} {...extra}>attr</button>
    </div>
  );
}

// No later source has the key: each handler is attached and runs, and Chrome reports NO_OWNER_EFFECT, covered by SC4001.
export function StillAttached() {
  const [s1] = createStore({ v: 0 });
  const earlier = () => {};
  const first: { onClick?: () => void } = { onClick: earlier };
  return (
    <div>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} title="t">attribute</button>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} {...{ onInput: earlier }}>spread</button>
      <button {...first} {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }}>earlier</button>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} onDblClick={earlier}>other key</button>
    </div>
  );
}

// A helper an overridden handler calls runs only if the handler does; the helper an attached handler calls runs unowned.
function startOverridden(s: { v: number }) { createRenderEffect(() => s.v, () => {}); }
function startAttached(s: { v: number }) { createRenderEffect(() => s.v, () => {}); }
export function Helpers() {
  const [s1] = createStore({ v: 0 });
  const extra: { onClick?: () => void } = { onClick: () => {} };
  return (
    <div>
      <button onClick={() => startOverridden(s1)} {...extra}>overridden</button>
      <button {...{ onClick: () => startOverridden(s1) }} onClick={undefined}>overridden spread</button>
      <button {...{ onClick: () => startAttached(s1) }}>attached</button>
    </div>
  );
}
