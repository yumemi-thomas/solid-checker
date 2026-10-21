import { createEffect, createSignal, runWithOwner } from "solid-js";

const [count] = createSignal(0);

export default function Page() {
  return null;
}

// Not read by `lazy`, which reads `default` only: its detached callback is dormant.
export function helper() {
  runWithOwner(null, () => {
    createEffect(count, () => {});
  });
}

// Read by name through `lazy(…, { export: "Named" })`: reachable, and its
// detached callback runs with no owner.
export function Named() {
  runWithOwner(null, () => {
    createEffect(count, () => {});
  });
  return null;
}
