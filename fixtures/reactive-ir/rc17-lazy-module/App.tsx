// `lazyModule(fn, moduleUrl?)` on solid-js@2.0.0-rc.17 returns `read = load`,
// and `load` is `if (p) return p; const cur = p = fn(); …`
// (`dist/solid.dev.js:1408-1431`, `dist/server.js:2268-2272`): `fn` runs only
// when the result (or its `preload`) is first called, on that caller's stack,
// with nothing set up around it. So what `fn` creates is owned exactly as that
// invocation is, as for `lazy`'s loader.
import { createEffect, createSignal, lazyModule } from "solid-js";
import * as Solid from "solid-js";

const [count, setCount] = createSignal(0);

// --- Owners are the invoker's -------------------------------------------------

// SC4001 (violation): invoked at module scope, where no owner is current
// (probed: NO_OWNER_EFFECT).
const atModule = lazyModule(() => {
  createEffect(() => count(), () => {});
  return import("./routes");
});
atModule();

// SC4001 (violation): the same through `preload`.
const preloaded = lazyModule(() => {
  createEffect(() => count(), () => {});
  return import("./routes");
});
preloaded.preload();

// SC4001 (violation): the same through a namespace import.
const namespaced = Solid.lazyModule(() => {
  createEffect(() => count(), () => {});
  return import("./routes");
});
namespaced();

// None: invoked in a component body, which owns the effect.
const inBody = lazyModule(() => {
  createEffect(() => count(), () => {});
  return import("./routes");
});
function Body() {
  inBody();
  return null;
}

// None: never invoked, so `fn` never runs.
lazyModule(() => {
  createEffect(() => count(), () => {});
  return import("./routes");
});

// --- Reads and writes are not claimed ----------------------------------------

// Not claimed: `fn` runs in its invoker's strict-read window and under its
// owned-scope guard (probed: dev warns STRICT_READ_UNTRACKED and throws
// REACTIVE_WRITE_IN_OWNED_SCOPE here), but only on the first invocation,
// which this vocabulary does not place.
const readsAndWrites = lazyModule(() => {
  console.log(count());
  setCount(1);
  return import("./routes");
});
function ReadWriteBody() {
  readsAndWrites();
  return null;
}

export function App() {
  return (
    <>
      <Body />
      <ReadWriteBody />
    </>
  );
}
