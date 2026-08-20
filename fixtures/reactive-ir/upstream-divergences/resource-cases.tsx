// createReaction runs invalidation under its own disposing computation in
// Solid 1.x. A resource created there is therefore owned and removed before
// the next invalidation, rather than leaking from a leaf callback. The
// returned track function is invoked so both the callback and its enclosed
// resource are proven reachable while remaining clear of SC3002.
import { createReaction, createResource, createSignal } from "solid-js";

export function TrackOnce() {
  const [count] = createSignal(0);
  const track = createReaction(() => {
    createResource(() => Promise.resolve(1));
  });
  track(() => count());
  return null;
}
