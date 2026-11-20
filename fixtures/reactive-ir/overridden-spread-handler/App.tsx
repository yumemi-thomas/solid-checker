import { createRenderEffect, createStore } from "solid-js";

// ADR 1140: rc.21's compiled `spread` reads an element's spreads and attributes as one ordered list of sources and skips a key a
// later source has (`collectProps`, `sourceHas`), so a literal-spread handler a later source overrides is never attached and never
// runs. Its SC4001 rows are obligations (`spread-handler-overridden`), never violations.
export function Overridden() {
  const [s1] = createStore({ v: 0 });
  const later = () => {};
  return (
    <div>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} onClick={later}>attribute</button>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} {...{ onClick: later }}>spread</button>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} onClick={undefined}>undefined</button>
    </div>
  );
}

// Violations still: no later source has the key, so each handler is attached and runs.
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

// The attribute handler a later spread overrides.
export function AttributeOverridden() {
  const [s1] = createStore({ v: 0 });
  const extra: { onClick?: () => void } = { onClick: () => {} };
  return <button onClick={() => { createRenderEffect(() => s1.v, () => {}); }} {...extra}>attr</button>;
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
