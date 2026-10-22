import { createEffect, createSignal, runWithOwner } from "solid-js";

const [count] = createSignal(0);

// Read by name from App.tsx (`const { reached } = await import("./other")`): a
// detached callback that runs with no owner.
export function reached() {
  runWithOwner(null, () => {
    createEffect(count, () => {});
  });
}

// Nothing reads it, by name or through the namespace: dormant in a closed program.
export function neverCalled() {
  runWithOwner(null, () => {
    createEffect(count, () => {});
  });
}
