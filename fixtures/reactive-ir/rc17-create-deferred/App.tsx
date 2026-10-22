// `createDeferred(fn, options?)` on solid-js@2.0.0-rc.17 is
// `accessor(computed(prev => { …; r = fn(prev); … }, options))`
// (@solidjs/signals `dist/dev.js:8897-8909`; solid-js `dist/solid.dev.js:975-976`
// wraps it for hydration as `createMemo` is, and `dist/server.js:2035` is
// `const createDeferred = createMemo;`). `fn` is a memo compute: it runs during
// the call under the node it creates, tracked, and its accessor is a memo's.
// Each case beside its `createMemo` twin, which answers the same.
import { createDeferred, createEffect, createMemo, createSignal } from "solid-js";
import { createDeferred as deferred } from "solid-js";
import * as Solid from "solid-js";

const [count, setCount] = createSignal(0);

// --- A write in `fn` meets the node's owned-scope guard ---------------------

// SC2001 (violation): probed, dev throws REACTIVE_WRITE_IN_OWNED_SCOPE.
function WriteInDeferred() {
  createDeferred(() => setCount(1));
  return null;
}

// SC2001 (violation): the createMemo twin.
function WriteInMemo() {
  createMemo(() => setCount(2));
  return null;
}

// SC2001 (violation): through a namespace import.
function NamespaceWrite() {
  Solid.createDeferred(() => setCount(3));
  return null;
}

// SC2001 (violation): through an aliased import.
function AliasWrite() {
  deferred(() => setCount(4));
  return null;
}

// --- Reads in `fn` subscribe the node -----------------------------------------

// None: a read in the compute is tracked (probed: it re-runs on a write).
function ReadInCompute() {
  const doubled = createDeferred(() => count() * 2);
  return <p>{doubled()}</p>;
}

// --- The accessor is a memo's -------------------------------------------------

// SC1001 (violation): a read of the accessor in a component body warns
// STRICT_READ_UNTRACKED (probed: once, as the memo's does).
function ReadAccessorInBody() {
  const doubled = createDeferred(() => count() * 2);
  console.log(doubled());
  return null;
}

// SC1001 (violation): the createMemo twin.
function ReadMemoInBody() {
  const doubled = createMemo(() => count() * 2);
  console.log(doubled());
  return null;
}

// --- `fn` owns what it creates ------------------------------------------------

// None: an effect created in the compute is owned by the node (probed: no
// NO_OWNER_EFFECT), even at module scope.
const owned = createDeferred(() => {
  createEffect(() => count(), () => {});
  return count();
});

export function App() {
  return (
    <>
      <WriteInDeferred />
      <WriteInMemo />
      <NamespaceWrite />
      <AliasWrite />
      <ReadInCompute />
      <ReadAccessorInBody />
      <ReadMemoInBody />
      <p>{owned()}</p>
    </>
  );
}
