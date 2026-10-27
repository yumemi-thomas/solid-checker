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
//
// A helper that calls the callback in its own body and also runs it elsewhere
// (hands it to a root or an owner, a microtask, a store) has runs the climb does
// not follow, and a legal write needs every run legal. rc.17 throws in the other
// run of each (checked on dev), so none may be clean. Uncertifiable: NullBoth,
// NullBothOwner, NullUntrackBoth, NullIifeBoth, HandlerBoth, CleanupNowAndLater,
// CleanupNowAndKeep, NullNowAndKeep, NullShorthandStore. Violation (its in place
// run is under the owner, and one throwing run is enough): CreatedOwnerBoth.
// CreatedOwnerReassign is no violation: the helper replaces the parameter before
// it calls it, so the callback never runs.
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

function both(cb: () => void): void {
  cb();
  createRoot(() => cb());
}
function bothOwner(cb: () => void): void {
  cb();
  runWithOwner(createOwner(), cb);
}
function nowAndLater(cb: () => void): void {
  cb();
  queueMicrotask(cb);
}
let saved: (() => void) | undefined;
function nowAndKeep(cb: () => void): void {
  cb();
  saved = cb;
}
export function fireSaved() {
  saved?.();
}

export function NullBoth() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    both(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function NullBothOwner() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    bothOwner(() => setV(2));
  });
  return <div>{v()}</div>;
}
export function NullUntrackBoth() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    untrack(() => both(() => setV(1)));
  });
  return <div>{v()}</div>;
}
export function NullIifeBoth() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    (() => both(() => setV(4)))();
  });
  return <div>{v()}</div>;
}
export function HandlerBoth() {
  const [v, setV] = createSignal(0);
  const reset = () => both(() => setV(2));
  return <button onClick={reset}>{v()}</button>;
}
export function CleanupNowAndLater() {
  const [v, setV] = createSignal(0);
  onCleanup(() => {
    nowAndLater(() => runWithOwner(createOwner(), () => setV(3)));
  });
  return <div>{v()}</div>;
}
export function CleanupNowAndKeep() {
  const [v, setV] = createSignal(0);
  onCleanup(() => {
    nowAndKeep(() => runWithOwner(createOwner(), () => setV(4)));
  });
  return <div>{v()}</div>;
}
export function NullNowAndKeep() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    nowAndKeep(() => setV(5));
  });
  return <div>{v()}</div>;
}

const sink: Array<{ cb: () => void }> = [];
function storeShorthand(cb: () => void): void {
  cb();
  sink.push({ cb });
}
function reassign(cb: () => void): void {
  cb = () => {};
  cb();
}

export function CreatedOwnerBoth() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    both(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function NullShorthandStore() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    storeShorthand(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function CreatedOwnerReassign() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    reassign(() => setV(1));
  });
  return <div>{v()}</div>;
}
