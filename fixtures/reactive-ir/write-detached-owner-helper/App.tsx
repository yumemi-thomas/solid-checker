import { createOwner, createRoot, createSignal, getOwner, onCleanup, runWithOwner, untrack } from "solid-js";

declare const attached: boolean;

// ADR 0457. A literal a `runWithOwner` callback hands to a function that calls
// that parameter in its own synchronous body on every completion runs on the
// call's stack and under the call's owner.
//
// Clean (rc.17 dev: the write lands): NullHelper (fuzzer seed 3176299446,
// Chrome-confirmed false positive before: the climb ended at the helper and the
// write took the component body's role), NullHelperNested, NullUntrackHelper;
// a proven fresh stack (CreatedOwnerHelperLater) and a disposal
// (CleanupCreatedOwnerHelper).
// Violations (rc.17 throws REACTIVE_WRITE_IN_OWNED_SCOPE): CreatedOwnerHelper,
// ComponentOwnerHelper.
// Uncertifiable: an owner that may be null (MaybeNullOwnerHelper, a violation
// before); a helper whose timing is not proven inline ends the climb unplaced,
// never at the outer role: it keeps the callback (NullHelperKeeps), calls it in
// a root it creates (NullHelperOwnRoot, a cost: rc.17 throws) or hands it on
// (CreatedOwnerHelperDetaches, legal).
function callNow<T>(cb: () => T): T {
  return cb();
}
function callLater(cb: () => void): void {
  setTimeout(cb, 0);
}
let kept: (() => void) | undefined;
function keep(cb: () => void): void {
  kept = cb;
}
function inOwnRoot(cb: () => void): void {
  createRoot(() => cb());
}
function detach(cb: () => void): void {
  runWithOwner(null, cb);
}

export function NullHelper() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    callNow(() => {
      return setV(17);
    });
  });
  return <div>{v()}</div>;
}
export function NullHelperNested() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    callNow(() => callNow(() => setV(1)));
  });
  return <div>{v()}</div>;
}
export function NullUntrackHelper() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    untrack(() => callNow(() => setV(1)));
  });
  return <div>{v()}</div>;
}
export function CreatedOwnerHelper() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    callNow(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function ComponentOwnerHelper() {
  const [v, setV] = createSignal(0);
  const owner = getOwner();
  runWithOwner(owner, () => {
    callNow(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function MaybeNullOwnerHelper() {
  const [v, setV] = createSignal(0);
  const owner = attached ? getOwner() : null;
  runWithOwner(owner, () => {
    callNow(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function CreatedOwnerHelperLater() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    callLater(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function NullHelperKeeps() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    keep(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function NullHelperOwnRoot() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    inOwnRoot(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function CreatedOwnerHelperDetaches() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    detach(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function CleanupCreatedOwnerHelper() {
  const [v, setV] = createSignal(0);
  onCleanup(() => {
    runWithOwner(createOwner(), () => {
      callNow(() => setV(1));
    });
  });
  return <div>{v()}</div>;
}
export function fireKept() {
  kept?.();
}
