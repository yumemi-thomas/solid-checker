// SC2005 until-in-tracked-scope. The dev build's until guard throws "Cannot call
// until inside a reactive scope" when an observer is active. The violation
// calls until inside a memo's compute on a re-run that a timer triggers,
// discarding the promise; the control awaits until from an event handler.
import { createMemo, createSignal, until } from "solid-js";

const [ready] = createSignal(true);

export function Violation() {
  const [trigger, setTrigger] = createSignal(false);
  const label = createMemo(() => {
    if (trigger()) void until(() => ready());
    return trigger();
  });
  setTimeout(() => setTrigger(true), 0);
  return <div>{String(label())}</div>;
}

export function Control() {
  return (
    <button
      onClick={async () => {
        await until(() => ready());
      }}
    >
      until
    </button>
  );
}
