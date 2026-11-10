import { createReaction, createSignal } from "solid-js";
import { trackShared, useMade, useTracker } from "./helpers";

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

// A hook that returns the tracker it makes: owned where the hook is called in
// the body, ownerless where a handler calls it (each `track()` there warns).
export function HookCalls() {
  const fromBody = useMade();
  return (
    <button
      onClick={() => {
        fromBody(() => count());
        const fromHandler = useMade();
        fromHandler(() => count());
      }}
    >
      go
    </button>
  );
}

// ADR 0931: a hook's call, or the creating call itself, called at once
// (`useArrow()(…)`): the tracker is that call's value, with no binding on the
// way. Owned from the body (silent); from the handler each call warns at the
// `track()` call, through an arrow hook, a creating call, a hook returning
// its binding. A hook a `let` holds may be replaced: an obligation there.
const useArrow = () => createReaction(() => {});
let useLet = () => createReaction(() => {});
export function ImmediateCalls() {
  useArrow()(() => count());
  return (
    <button
      onClick={() => {
        useArrow()(() => count());
        createReaction(() => {})(() => count());
        useMade()(() => count());
        useLet()(() => count());
        useLet = useArrow;
      }}
    >
      go
    </button>
  );
}
