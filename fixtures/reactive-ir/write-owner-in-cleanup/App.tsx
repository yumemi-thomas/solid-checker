import {
  action,
  createEffect,
  createOwner,
  createRoot,
  createSignal,
  getOwner,
  onCleanup,
  runWithOwner,
} from "solid-js";

// The write guard stands down while the runtime disposes an owner
// (@solidjs/signals@2.0.0-rc.17 `dist/dev-shared.js:5620-5626`, `disposalDepth`,
// raised by `runDisposal` at `:1314`), so a write that an `onCleanup` callback
// runs under a proven owner lands without throwing. The same call outside a
// cleanup throws (ADR 0455 amendment).

// --- Legal: the callback runs only in a disposal ----------------------------

// A component's own owner, captured in the body.
export function CleanupComponentOwner() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => runWithOwner(o, () => setV(1)));
  return <div>{v()}</div>;
}
// An owner the call creates.
export function CleanupCreatedOwner() {
  const [v, setV] = createSignal(0);
  onCleanup(() => runWithOwner(createOwner(), () => setV(2)));
  return <div>{v()}</div>;
}
// Through an inline standard-library callback.
export function CleanupOwnerForEach() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => runWithOwner(o, () => [1].forEach(() => setV(3))));
  return <div>{v()}</div>;
}
// A root the cleanup creates: its body runs during the cleanup.
export function CleanupCreateRoot() {
  const [v, setV] = createSignal(0);
  onCleanup(() => createRoot(() => setV(4)));
  return <div>{v()}</div>;
}

// --- Uncertifiable: a named function a cleanup may call --------------------

// The function runs where it is called. A cleanup hands it on or calls it, but
// nothing here reads its other callers: neither a violation nor legal.
export function CleanupNamedOwnerHelper() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const f = () => runWithOwner(o, () => setV(10));
  onCleanup(f);
  return <div>{v()}</div>;
}
export function CleanupCallsOwnerHelper() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  const f = () => runWithOwner(o, () => setV(11));
  onCleanup(() => f());
  return <div>{v()}</div>;
}
export function CleanupCallsRootHelper() {
  const [v, setV] = createSignal(0);
  const g = () => createRoot(() => setV(12));
  onCleanup(() => g());
  return <div>{v()}</div>;
}

// --- Violations: the same owner outside a disposal --------------------------

// An effect apply's returned cleanup is no registration: a rerun calls it
// outside `runDisposal`, and the write throws.
export function EffectReturnedCleanup() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  createEffect(
    () => v(),
    () => () => runWithOwner(o, () => setV(6))
  );
  return <div>{v()}</div>;
}
// A cleanup that schedules the write runs it after the disposal is over.
export function CleanupSchedulesWrite() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {
    setTimeout(() => runWithOwner(o, () => setV(7)));
  });
  return <div>{v()}</div>;
}
// A cleanup registered, but the write is written in the body.
export function CleanupRegisteredWriteInBody() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {});
  runWithOwner(o, () => setV(8));
  return <div>{v()}</div>;
}
// A cleanup that only builds the function: it runs where it is called.
export function CleanupStoresFunction() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  let later: (() => void) | undefined;
  onCleanup(() => {
    later = () => runWithOwner(o, () => setV(9));
  });
  return <button onClick={() => later?.()}>{v()}</button>;
}
// An action call's guard does not stand down in a disposal.
const act = action(function* () {});
export function CleanupActionUnderOwner() {
  const o = getOwner();
  onCleanup(() => runWithOwner(o, () => act()));
  return <div />;
}
