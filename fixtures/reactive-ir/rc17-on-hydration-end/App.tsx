// `onHydrationEnd(callback)` on solid-js@2.0.0-rc.17 is
// `queueMicrotask(callback)` outside a hydration and otherwise a push that
// `drainHydrationCallbacks` runs where hydration ends (`dist/solid.dev.js:385-404`);
// the server build only queues it (`dist/server.js:2731-2733`). It never runs
// during the call, and it has an owner only when the drain does: probed, `null`
// from the microtask, and `hydrate`'s caller's owner from the drain at the end
// of `hydrate` (a root, when `hydrate` was called in one).
import { createEffect, createSignal, onCleanup, onHydrationEnd } from "solid-js";
import * as Solid from "solid-js";

const [count, setCount] = createSignal(0);

// --- Owners: with or without one, so nothing is claimed -----------------------

// Not a violation: unowned from the microtask (probed: NO_OWNER_EFFECT), owned
// by the root around `hydrate` when the drain runs it there.
onHydrationEnd(() => {
  createEffect(() => count(), () => {});
});

// Not a violation: the same for `onCleanup` in a component body.
function CleanupInBody() {
  onHydrationEnd(() => {
    onCleanup(() => {});
  });
  return null;
}

// Not a violation: the same through a namespace import.
Solid.onHydrationEnd(() => {
  createEffect(() => count(), () => {});
});

// SC4001 (violation): the control, the same effect created at module scope.
createEffect(() => count(), () => {});

// --- Reads and writes leave the caller's scope -------------------------------

// None: the callback runs after the body returns, outside its strict-read
// window and its owned-scope guard (probed: no STRICT_READ_UNTRACKED, and the
// write is legal from the microtask and from a module-scope `hydrate`'s
// drain). A drain under a root would throw; that write is a known miss.
function ReadAndWriteInBody() {
  onHydrationEnd(() => {
    console.log(count());
    setCount(1);
  });
  return null;
}

export function App() {
  return (
    <>
      <CleanupInBody />
      <ReadAndWriteInBody />
    </>
  );
}
