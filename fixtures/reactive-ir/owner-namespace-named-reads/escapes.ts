import { createEffect, createSignal, runWithOwner } from "solid-js";

const [count] = createSignal(0);

// The namespace of this module escapes unnamed (App.tsx passes it to
// `Object.keys`): a later computed read could reach this export, so whether its
// detached callback runs is unproven. Uncertifiable, not a violation (ADR 0347 amendment).
export function escaped() {
  runWithOwner(null, () => {
    createEffect(count, () => {});
  });
}
