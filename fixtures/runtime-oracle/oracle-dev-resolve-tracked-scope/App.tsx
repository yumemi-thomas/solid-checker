// SC2004 resolve-in-tracked-scope. The dev build's resolve guard throws "Cannot
// call resolve inside a reactive scope" when an observer is active. The
// violation resolves inside a memo's compute on a re-run that a timer triggers,
// discarding the promise so the memo stays synchronous; the control resolves
// from an event handler, where no observer is active.
import { createMemo, createSignal, resolve } from "solid-js";

const [user] = createSignal("ada");

export function Violation() {
  const [trigger, setTrigger] = createSignal(false);
  const label = createMemo(() => {
    if (trigger()) void resolve(() => user());
    return trigger();
  });
  setTimeout(() => setTrigger(true), 0);
  return <div>{String(label())}</div>;
}

export function Control() {
  return (
    <button
      onClick={async () => {
        const current = await resolve(() => user());
        void current;
      }}
    >
      resolve
    </button>
  );
}
