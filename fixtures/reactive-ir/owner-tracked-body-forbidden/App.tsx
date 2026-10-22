import { createEffect, createSignal, createTrackedEffect, onCleanup, onSettled } from "solid-js";

const [count] = createSignal(0);

// A createTrackedEffect body always runs under the tracked node, which forbids
// children, so the onCleanup there throws CLEANUP_IN_FORBIDDEN_SCOPE (SC3001).
// The effect itself has no owner here (SC4001 at its callee), but its body is
// not NO_OWNER_CLEANUP.
createTrackedEffect(() => onCleanup(() => {}));

// Control: a queued onSettled at module scope runs with no owner, so its
// onCleanup is NO_OWNER_CLEANUP (SC4001).
onSettled(() => onCleanup(() => {}));

export function TimerTrackedEffect() {
  // The same body under an unowned timer callback: still the tracked node's claim.
  setTimeout(() => {
    createTrackedEffect(() => {
      onCleanup(() => {});
    });
  }, 5);
  return <i />;
}

export function TimerTrackedPrimitive() {
  // Control: a primitive in an unowned tracked body throws PRIMITIVE_IN_FORBIDDEN_SCOPE
  // (SC3001). No SC4001 is claimed for the primitive; the effect's own unowned
  // creation is the SC4001 claim.
  setTimeout(() => {
    createTrackedEffect(() => {
      createEffect(count, () => {});
    });
  }, 5);
  return <i />;
}

export function OwnedTrackedEffect() {
  // Control: an owned tracked effect's cleanup throws CLEANUP_IN_FORBIDDEN_SCOPE (SC3001).
  createTrackedEffect(() => {
    count();
    onCleanup(() => {});
  });
  return <i />;
}
