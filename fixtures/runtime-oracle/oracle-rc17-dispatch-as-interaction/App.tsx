// dispatchAsInteraction(event, fn) runs fn() inline in the caller's owner and
// listener (@solidjs/web@2.0.0-rc.17 dist/web.dev.js:653). A write inside it
// in a component body throws REACTIVE_WRITE_IN_OWNED_SCOPE exactly as the bare
// write does, and a read inside it warns STRICT_READ_UNTRACKED; the controls
// write from an event handler and read from a memo compute.
import { createMemo, createSignal } from "solid-js";
import { dispatchAsInteraction } from "@solidjs/web";

const [count, setCount] = createSignal(0);

export function BodyWrite() {
  dispatchAsInteraction(new Event("probe"), () => setCount(1));
  return <div />;
}

export function BodyRead() {
  dispatchAsInteraction(new Event("probe"), () => console.log(count()));
  return <div />;
}

export function Control() {
  const doubled = createMemo(() => dispatchAsInteraction(new Event("probe"), () => count()) * 2);
  return (
    <button onClick={(e) => dispatchAsInteraction(e, () => setCount(doubled()))}>
      {doubled()}
    </button>
  );
}
