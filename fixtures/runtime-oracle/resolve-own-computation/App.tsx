// ADR 0997: a `resolve` or `until` call in a callback its primitive runs elsewhere ends silent only where the dialect states that no observer is
// active there. The callback of `resolve` and of `until` is the compute of a node of the primitive's own (`watch(fn, …)`): the guard throws in it
// (Chrome rc.21, from a click handler), so a call there is an obligation that says so. `from`'s subscribe function runs where its iterator is
// started, inside the computation that consumes it (Chrome rc.21 throws in a memo): not stated, an obligation. `...R` is `resolve`, `...U` `until`.
import * as S from "solid-js";
import { createEffect, createMemo, createSignal, onCleanup, resolve, until, untrack } from "solid-js";

const [n, setN] = createSignal(1);
const resolveAlias = resolve;
const untilAlias = until;

// U (computation-error-unobserved): the callback of `resolve` is a computation of its own.
export function InResolveR() {
  return <button onClick={() => { void resolve(() => { void resolve(() => n()); return n(); }); }}>a</button>;
}
export function InResolveU() {
  return <button onClick={() => { void resolve(() => { void until(() => n() > 0); return n(); }); }}>b</button>;
}

// U: the predicate of `until` is one too.
export function InUntilR() {
  return <button onClick={() => { void until(() => { void resolve(() => n()); return n() > 0; }); }}>c</button>;
}
export function InUntilU() {
  return <button onClick={() => { void until(() => { void until(() => n() > 1); return n() > 0; }); }}>d</button>;
}

// U: the holder is the primitive by an alias and by the namespace member.
export function InAliasR() {
  return <button onClick={() => { void resolveAlias(() => { void resolve(() => n()); return n(); }); }}>e</button>;
}
export function InNamespaceU() {
  return <button onClick={() => { void S.until(() => { void S.until(() => n() > 1); return n() > 0; }); }}>f</button>;
}
export function InAliasU() {
  return <button onClick={() => { void untilAlias(() => { void until(() => n() > 1); return n() > 0; }); }}>g</button>;
}

// silent, slot-clears-observer: the call is written in `untrack`'s callback, which clears the observer the `resolve` callback around it set.
export function UntrackInResolveR() {
  return <button onClick={() => { void resolve(() => untrack(() => { void resolve(() => n()); return n(); })); }}>h</button>;
}

// silent, slot-runs-elsewhere: an effect's apply and a cleanup run with no observer (Chrome rc.21: quiet for the cleanup of a memo that re-runs).
export function InApplyR() {
  createEffect(() => n(), () => { void resolve(() => n()); });
  return <p>i</p>;
}
export function InCleanupR() {
  const m = createMemo(() => { onCleanup(() => { void resolve(() => n()); }); return n(); });
  return <button onClick={() => setN(n() + 1)}>{String(m())}</button>;
}
export function InCleanupU() {
  const m = createMemo(() => { onCleanup(() => { void until(() => n() > 0); }); return n(); });
  return <button onClick={() => setN(n() + 1)}>{String(m())}</button>;
}

// no site: a local function named like the primitive is not the primitive, and its callback is no computation.
export function ShadowedHolderR() {
  const resolve = (f: () => number) => f();
  return <button onClick={() => { void resolve(() => n()); }}>j</button>;
}
