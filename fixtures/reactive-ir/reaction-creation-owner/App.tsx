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

// ADR 0549's value flows: a slot of a closed literal, a hook's returned
// literal (destructured or read as a member), a tuple named by a `const`.
// Each still names the module-scope tracker: violations. Through a `let`:
// an obligation. The same shapes over a body-created tracker: silent.
const useShared = () => ({ track: trackShared });
export function ValueFlows() {
  const owned = createReaction(() => {});
  const { viaSlot } = { viaSlot: trackShared };
  const { track: viaHook } = useShared();
  const shared = useShared();
  const pair = [0, trackShared] as const;
  const [, viaTuple] = pair;
  let viaLet = trackShared;
  const { ownedSlot } = { ownedSlot: owned };
  const [, ownedTuple] = [0, owned] as const;
  return (
    <button
      onClick={() => {
        viaSlot(() => count());
        viaHook(() => count());
        shared.track(() => count());
        viaTuple(() => count());
        viaLet(() => count());
        ownedSlot(() => count());
        ownedTuple(() => count());
        viaLet = trackShared;
      }}
    >
      go
    </button>
  );
}
