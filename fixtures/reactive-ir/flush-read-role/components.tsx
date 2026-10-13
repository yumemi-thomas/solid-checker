import { createEffect, createMemo, createSignal, flush, untrack } from "solid-js";

const [count] = createSignal(0);

// `flush(fn)` runs `fn` inline and leaves the caller's listener current, so a
// read inside it has the caller's read role.

// SC1001 (violation): in a component body the read runs inside the
// strict-read window, exactly as a bare read there does.
function FlushInBody() {
  flush(() => console.log(count()));
  return null;
}

// None: a memo compute reading through `flush(fn)` subscribes the memo, and
// re-runs when `count` is written.
function FlushInMemo() {
  createMemo(() => flush(() => count()) * 2);
  return null;
}

// None: the same for an effect compute.
function FlushInEffectCompute() {
  createEffect(
    () => flush(() => count()),
    (value) => console.log(value),
  );
  return null;
}

// None: `untrack` clears the listener, inside a memo or a body, and a
// `flush(fn)` nested in it reads under the cleared listener.
function UntrackInMemo() {
  createMemo(() => untrack(() => flush(() => count())) * 2);
  untrack(() => console.log(count()));
  return null;
}

export function App() {
  return (
    <>
      <FlushInBody />
      <FlushInMemo />
      <FlushInEffectCompute />
      <UntrackInMemo />
    </>
  );
}
