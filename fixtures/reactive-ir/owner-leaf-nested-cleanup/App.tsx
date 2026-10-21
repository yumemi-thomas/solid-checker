import { createSignal, createTrackedEffect, onCleanup, onSettled } from "solid-js";

const [count] = createSignal(0);

export function NestedInTracked() {
  // An onSettled inside a tracked effect is queued out of band: its callback runs with no owner.
  createTrackedEffect(() => { count(); onSettled(() => { onCleanup(() => {}); }); });
  return <i />;
}

export function NestedInSettled() {
  // An onSettled inside an owned onSettled is queued out of band too.
  onSettled(() => { onSettled(() => { onCleanup(() => {}); }); });
  return <i />;
}

function registerSettled() {
  onSettled(() => { onCleanup(() => {}); });
}

export function HelperFromLeaf() {
  // The helper's onSettled is called from a leaf body, so it runs out of band.
  onSettled(() => { registerSettled(); });
  return <i />;
}

function cleanupHelper() {
  onCleanup(() => {});
}

export function HelperFromNestedLeaf() {
  // A helper called from a nested out-of-band onSettled runs with no owner.
  onSettled(() => { onSettled(() => { cleanupHelper(); }); });
  return <i />;
}

export function TrackedInTracked() {
  // Control: a tracked effect inside a tracked effect throws PRIMITIVE_IN_FORBIDDEN_SCOPE
  // before its callback runs, so only that throw is claimed, not the cleanup.
  createTrackedEffect(() => { count(); createTrackedEffect(() => { onCleanup(() => {}); }); });
  return <i />;
}

export function TrackedOwnCleanup() {
  // Control: an owned tracked effect's own cleanup throws CLEANUP_IN_FORBIDDEN_SCOPE (SC3001).
  createTrackedEffect(() => { count(); onCleanup(() => {}); });
  return <i />;
}

export function TrackedInSettled() {
  // Control: a tracked effect inside an owned onSettled throws, so its nested callback never runs.
  onSettled(() => { createTrackedEffect(() => { onSettled(() => { onCleanup(() => {}); }); }); });
  return <i />;
}
