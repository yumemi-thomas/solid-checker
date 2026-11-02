import { createReaction, createSignal } from "solid-js";
import { trackShared, useTracker } from "./helpers";

const [count, setCount] = createSignal(0);

// A tracker created at module scope runs its effect under no owner, wherever
// `track()` is called: each call warns NO_OWNER_EFFECT.
const trackModule = createReaction(() => {});
trackModule(() => count());

export function Uses() {
  // Created in the body: the body's owner, from the body or a handler.
  const trackOwned = createReaction(() => {});
  trackOwned(() => count());
  const fromHook = useTracker();
  // A `let` may hold another function by the time it is called: not followed.
  let trackLet = createReaction(() => {});
  return (
    <button
      onClick={() => {
        trackModule(() => count());
        trackShared(() => count());
        trackOwned(() => count());
        fromHook(() => count());
        trackLet(() => count());
        trackLet = () => {};
        setCount(1);
      }}
    >
      go
    </button>
  );
}
