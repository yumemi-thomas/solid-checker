import {
  createEffect,
  createRoot,
  createSignal,
  getOwner,
  onCleanup,
  runWithOwner,
} from "solid-js";

// The same cleanup shapes under a `@solidjs/signals` release the write guard
// answer is not known for. Inside a disposal the shape is neither legal nor a
// violation (uncertifiable); outside one the guard throws on every release.

// Uncertifiable: the cleanup runs in a disposal, and nothing says the guard
// stands down there.
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

// Violations: not in a disposal on any release.
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
// The effect tracks `n`, which a button changes, so a rerun calls the cleanup.
export function EffectReturnedCleanup() {
  const [v, setV] = createSignal(0);
  const [n, setN] = createSignal(0);
  const o = getOwner();
  createEffect(
    () => n(),
    () => () => runWithOwner(o, () => setV(5))
  );
  return <button onClick={() => setN(n() + 1)}>{v()}</button>;
}
