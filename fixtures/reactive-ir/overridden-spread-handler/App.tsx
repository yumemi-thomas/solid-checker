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
  return (
    <div>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} title="t">attribute</button>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} {...{ onInput: earlier }}>spread</button>
      <button onClick={earlier} {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }}>earlier</button>
      <button {...{ onClick: () => { createRenderEffect(() => s1.v, () => {}); } }} onclick={earlier}>case</button>
    </div>
  );
}

// The attribute handler a later spread overrides.
export function AttributeOverridden() {
  const [s1] = createStore({ v: 0 });
  const later = () => {};
  return <button onClick={() => { createRenderEffect(() => s1.v, () => {}); }} {...{ onClick: later }}>attr</button>;
}
