// createResource eagerly creates computations that need disposal, so the
// dialect's cleanup rule treats it like the other owner-creating primitives:
// allocating one inside createReaction's leaf onInvalidate scope is
// reported, exactly as a mapArray or createSelector there would be. The
// returned track function is invoked so the reaction is provably live.
import { createReaction, createResource, createSignal } from "solid-js";

export function TrackOnce() {
  const [count] = createSignal(0);
  const track = createReaction(() => {
    createResource(() => Promise.resolve(1));
  });
  track(() => count());
  return null;
}
