import { createEffect, createRenderEffect, createStore } from "solid-js";

// ADR 1132: the handler body of the dny-legal-* programs, on a button the oracle clicks. Chrome reports NO_OWNER_EFFECT at the
// `createEffect` (SC4001 missing-owner, a violation): the reading of "wrong if it runs" the judge grants those undriven programs.
export function Driven() {
  const [s1] = createStore({ v: 0 });
  return (
    <div>
      <button onClick={() => {
        { const f = () => s1.v; createEffect(f, () => {}); f(); };
      }}>go</button>
    </div>
  );
}

// The same handler on a button disabled from its mount (dny-legal-mount-disabled): no event reaches it, Chrome is quiet, and the
// checker's SC4001 violation there is a false positive (held pending a fix in scripts/fuzz/unexecuted-known.json).
export function MountDisabled() {
  const [s1] = createStore({ v: 0 });
  return (
    <div>
      <button disabled onClick={() => {
        { const f = () => s1.v; createEffect(f, () => {}); f(); };
      }}>go</button>
    </div>
  );
}

// The same with a render effect, driven: Chrome reports NO_OWNER_EFFECT at `createRenderEffect` too.
export function DrivenRender() {
  const [s1] = createStore({ v: 0 });
  return (
    <div>
      <button onClick={() => {
        { const f = () => s1.v; createRenderEffect(f, () => {}); f(); };
      }}>go</button>
    </div>
  );
}
