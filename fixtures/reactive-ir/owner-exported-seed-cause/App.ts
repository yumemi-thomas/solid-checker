import { onCleanup } from "solid-js";

// The project is open, so an exported helper is seeded with a caller this
// build cannot see, one that may supply no owner (its open-world seed).

// Exported-caller cause: the cleanup is in the exported helper's own body, and
// the seed is its owner edge. rc.17 warns NO_OWNER_CLEANUP for an unowned
// caller and registers the cleanup for an owned one.
export function useRelease() {
  onCleanup(() => {});
}

// Unplaced-callback cause (ADR 0534): the cleanup is in a callback the
// exported helper hands to a scheduler; no owner edge runs it, whatever the
// helper's callers supply.
function later(callback: () => void) {
  setTimeout(callback, 0);
}

export function useDeferredRelease() {
  later(() => {
    onCleanup(() => {});
  });
}

// The same clause for a callback `forEach` runs in place: the owner graph has no
// edge for it (context zero), so it is ADR 0534's unplaced callback too, not a
// moved exported-caller cause (before ADR 0534 it was silent).
export function useReleaseInPlace() {
  [1].forEach(() => {
    onCleanup(() => {});
  });
}
