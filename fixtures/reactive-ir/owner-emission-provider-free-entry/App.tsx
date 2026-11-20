import { createContext, createEffect, getOwner, useContext } from "solid-js";
import { useHookValue } from "./Hook";

// ADR 1142: a provider-free context read (`createContext<T>()`, no provider anywhere) in a layer the owner graph shows an owned context for is SC4002's claim for that
// run (the owned run throws ContextNotFoundError). The owned context is the edges the graph holds (ADR 0868), and a layer entered by a route none shows (a reference
// carried on, stored, handed to a call nothing follows) may also run the read with no owner, where Solid throws NO_OWNER_CONTEXT before it looks for a provider: SC4001's
// obligation beside the claim. The arm that decides a read WITH a default asked the entry test already (`root-owned-entry-open`, `named-layer-entry-open`); the
// provider-free arm did not, so a carried function's second code had no row at all.
const NoDefault = createContext<number>();
const WithDefault = createContext<number>(1);

// The defect (fuzz: mx-census CONTEXT_NOT_FOUND `barm-carried-handler`). The effect's compute runs `f1` under the effect's owner (ContextNotFoundError); the click's
// `forEach` carries `f1` on and calls it from the handler, with no owner (NO_OWNER_CONTEXT, Chrome rc.21). SC4002 violation (the owned run) and SC4001 obligation.
export function CarriedHandler() {
  const f1 = () => {
    useContext(NoDefault);
  };
  createEffect(f1, () => {});
  return <button onClick={() => [f1].forEach((g) => g())}>go</button>;
}

// The same carrier in the body: the carried call runs `f2` under the body's owner. The owner pass does not follow a call through the carrier's parameter, so it cannot say
// where the call runs (the named cost: ADR 1124's census places it for the read roles, not for the owner graph). Same rows.
export function CarriedBody() {
  const f2 = () => {
    useContext(NoDefault);
  };
  createEffect(f2, () => {});
  [f2].forEach((g) => g());
  return <p>body</p>;
}

// Entry routes the graph holds no edge for. Stored in a variable a handler calls (Chrome: NO_OWNER_CONTEXT at the click).
let kept: (() => void) | undefined;
export function Stored() {
  const f3 = () => {
    useContext(NoDefault);
  };
  createEffect(f3, () => {});
  kept = f3;
  return <button onClick={() => kept?.()}>go</button>;
}

// Handed to a project helper that calls it (the helper's parameter holds no edge).
function run(callback: () => void) {
  callback();
}
export function Helper() {
  const f4 = () => {
    useContext(NoDefault);
  };
  createEffect(f4, () => {});
  return <button onClick={() => run(f4)}>go</button>;
}

// Handed to a timer: it runs later on its own stack, with no owner.
export function Timer() {
  const f5 = () => {
    useContext(NoDefault);
  };
  createEffect(f5, () => {});
  setTimeout(f5);
  return <p>timer</p>;
}

// A member of an object a handler calls.
export function Member() {
  const f6 = () => {
    useContext(NoDefault);
  };
  createEffect(f6, () => {});
  const box = { run: f6 };
  return <button onClick={() => box.run()}>go</button>;
}

// No region around the read: the body calls `f7` (an owned edge) and a handler carries it on. The owned context comes from the call edge, not from a providing region.
export function OwnedCallCarried() {
  const f7 = () => {
    useContext(NoDefault);
  };
  f7();
  return <button onClick={() => [f7].forEach((g) => g())}>go</button>;
}

// An imported function used as the effect's compute and carried on in a handler: the reference that escapes is in another file than the read (Hook.tsx).
export function ImportedHook() {
  createEffect(useHookValue, () => {});
  return <button onClick={() => [useHookValue].forEach((g) => g())}>go</button>;
}

// Controls: nothing carries the function on, so the effect is its only entry and its owner is current at every run of the read. SC4002 only.
export function EntryOnly() {
  const f8 = () => {
    useContext(NoDefault);
  };
  createEffect(f8, () => {});
  return <p>only</p>;
}

// `[1].forEach(f9)` runs `f9` on the body's stack, under the body's owner (an inline run, not a carried one). SC4002 only.
export function InlineBody() {
  const f9 = () => {
    useContext(NoDefault);
  };
  [1].forEach(f9);
  return <p>inline</p>;
}

// The read is under `if (getOwner())` in the same invocation: the handler's run, with no owner, skips it. SC4002 only (the owned run).
export function Probed() {
  const f10 = () => {
    if (getOwner()) {
      useContext(NoDefault);
    }
  };
  createEffect(f10, () => {});
  return <button onClick={() => [f10].forEach((g) => g())}>go</button>;
}

// A context WITH a default is the other arm (it asked the entry test before this change): SC4001 obligation, no SC4002 (the owned run returns the default).
export function Defaulted() {
  const f11 = () => {
    useContext(WithDefault);
  };
  createEffect(f11, () => {});
  return <button onClick={() => [f11].forEach((g) => g())}>go</button>;
}
