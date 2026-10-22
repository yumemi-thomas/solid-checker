import { createEffect, createSignal, runWithOwner } from "solid-js";

const [count] = createSignal(0);

// Read as a member of an awaited import in App.tsx: handed out by name, so its
// detached callback runs with no owner.
export function handedOut() {
  runWithOwner(null, () => {
    createEffect(count, () => {});
  });
}

// Never read: dormant in a closed program.
export function unread() {
  runWithOwner(null, () => {
    createEffect(count, () => {});
  });
}
