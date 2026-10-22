// `dispatchAsInteraction(event, fn)` on @solidjs/web@2.0.0-rc.17 is
// `return fn();` in production (`dist/web.js:484`) and
// `OBSERVE.attribution.withInteraction({…}, fn)` in dev (`dist/web.dev.js:653`),
// which runs `fn()` inline between the attribution engine's frame hooks
// (@solidjs/signals `dist/dev-shared.js:335-347`). `fn` keeps the caller's
// owner and listener, so code in it has the caller's write and read roles,
// as `flush(fn)`'s does.
import { createEffect, createMemo, createSignal, untrack } from "solid-js";
import { dispatchAsInteraction } from "@solidjs/web";
import * as Web from "@solidjs/web";

const [count, setCount] = createSignal(0);
const event = new Event("probe");

// --- Writes have the caller's role ------------------------------------------

// SC2001 (violation): a component body runs under a root in dev, and the
// callback keeps it as the owner, exactly as a bare write there.
function WriteInBody() {
  dispatchAsInteraction(event, () => setCount(1));
  return null;
}

// The same through a namespace import.
function NamespaceWriteInBody() {
  Web.dispatchAsInteraction(event, () => setCount(2));
  return null;
}

// The same with the callback passed by name.
const bump = () => setCount(3);
function NamedWriteInBody() {
  dispatchAsInteraction(event, bump);
  return null;
}

// SC2001 (violation): inside a memo compute.
function WriteInMemo() {
  createMemo(() => dispatchAsInteraction(event, () => setCount(4)));
  return null;
}

// None: an event handler has no owner, so the write is legal there.
function Handler() {
  return <button onClick={(e) => dispatchAsInteraction(e, () => setCount(5))} />;
}

// None: at module scope, the same.
dispatchAsInteraction(event, () => setCount(6));

// --- Reads have the caller's role -------------------------------------------

// SC1001 (violation): in a component body the read runs inside the
// strict-read window, exactly as a bare read there does (probed: dev warns
// `STRICT_READ_UNTRACKED` once, as for the bare read).
function ReadInBody() {
  dispatchAsInteraction(event, () => console.log(count()));
  return null;
}

// None: a memo compute reading through it subscribes the memo (probed: the
// memo re-runs when `count` is written).
function ReadInMemo() {
  createMemo(() => dispatchAsInteraction(event, () => count()) * 2);
  return null;
}

// None: `untrack` clears the listener, and the nested callback reads under it.
function ReadInUntrack() {
  untrack(() => dispatchAsInteraction(event, () => console.log(count())));
  return null;
}

// --- Owners are the caller's -------------------------------------------------

// SC4001 (violation): at module scope there is no owner to inherit.
dispatchAsInteraction(event, () => createEffect(() => count(), () => {}));

// None: in a component body the effect is owned by the component.
function EffectInBody() {
  dispatchAsInteraction(event, () => createEffect(() => count(), () => {}));
  return null;
}

export function App() {
  return (
    <>
      <WriteInBody />
      <NamespaceWriteInBody />
      <NamedWriteInBody />
      <WriteInMemo />
      <Handler />
      <ReadInBody />
      <ReadInMemo />
      <ReadInUntrack />
      <EffectInBody />
    </>
  );
}
