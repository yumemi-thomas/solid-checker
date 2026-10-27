import {
  createEffect,
  createRoot,
  createSignal,
  getOwner,
  onCleanup,
  runWithOwner,
} from "solid-js";

// @solidjs/signals@2.0.0-rc.13 has no `!disposalDepth` test in its write
// guards, so the write throws inside a disposal too (ADR 0455 amendment).

// Violations: the guard rejects in a disposal.
export function CleanupRunWithOwner() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => runWithOwner(o, () => setV(1)));
  return <div>{v()}</div>;
}
export function CleanupCreateRoot() {
  const [v, setV] = createSignal(0);
  onCleanup(() => createRoot(() => setV(2)));
  return <div>{v()}</div>;
}

// Violations: not in a disposal.
export function BodyWrite() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(() => {});
  runWithOwner(o, () => setV(3));
  return <div>{v()}</div>;
}
export function AwaitedCleanup() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  onCleanup(async () => {
    await 0;
    runWithOwner(o, () => setV(4));
  });
  return <div>{v()}</div>;
}
export function EffectReturnedCleanup() {
  const [v, setV] = createSignal(0);
  const o = getOwner();
  createEffect(
    () => v(),
    () => () => runWithOwner(o, () => setV(5))
  );
  return <div>{v()}</div>;
}
