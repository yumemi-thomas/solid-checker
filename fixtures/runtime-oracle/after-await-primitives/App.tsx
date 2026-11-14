import { createMemo, createRoot, createSignal, getOwner, runWithOwner, untrack } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B follow-up (ADR 0976), R5 of the re-check of #233: a read after the `await` of an async computation, inside the callback of a primitive that runs it on this
// stack. Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT for `runWithOwner(null, …)` and for `runWithOwner(getOwner(), …)` taken after the `await` (a continuation has no owner), and
// nothing for `runWithOwner(owner, …)` with the owner captured before the `await`, for `untrack(…)` and for `createRoot(…)`. The functions are declarations handed by identifier to a
// project wrapper that gives its parameter to `createMemo` (a start the producers' index does not follow). Each call is written on one line.
function reader() {
  return source();
}
function track(compute: () => Promise<unknown>) {
  return createMemo(compute);
}
async function viaNull() {
  await pause();
  return runWithOwner(null, () => source());
}
async function viaOwnerAfter() {
  await pause();
  return runWithOwner(getOwner(), () => source());
}
async function viaReference() {
  await pause();
  return runWithOwner(null, reader);
}
async function viaCaptured() {
  const owner = getOwner();
  await pause();
  return runWithOwner(owner, () => source());
}
async function viaUntrack() {
  await pause();
  return untrack(() => source());
}
async function viaRoot() {
  await pause();
  return createRoot(() => source());
}

createRoot(() => {
  track(viaNull);
  track(viaOwnerAfter);
  track(viaReference);
  track(viaCaptured);
  track(viaUntrack);
  track(viaRoot);
});

export function Probe() {
  return <p>probe</p>;
}
