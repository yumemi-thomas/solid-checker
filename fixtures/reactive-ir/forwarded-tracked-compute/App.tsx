import { createSignal } from "solid-js";
import { forwarded, tracked, trackedAndEager, trackedLater, untracked } from "./wrappers";

// ---- Clean: the literal runs only as a memo's tracked compute.

export function UsesTracked() {
  const [count] = createSignal(0);
  const doubled = tracked(() => count() * 2);
  return <main>{doubled()}</main>;
}

// ---- Not proven tracked.

export function UsesTrackedAndEager() {
  const [count] = createSignal(0);
  const doubled = trackedAndEager(() => count() * 2);
  return <main>{doubled()}</main>;
}
export function UsesUntracked() {
  const [count] = createSignal(0);
  const value = untracked(() => count() * 2);
  return <main>{value}</main>;
}
export function UsesTrackedLater() {
  const [count] = createSignal(0);
  void trackedLater(() => count() * 2);
  return <main>a</main>;
}
export function UsesForwarded() {
  const [count] = createSignal(0);
  const doubled = forwarded(() => count() * 2);
  return <main>{doubled()}</main>;
}
