// SC2002 action-called-in-owned-scope. The dev build's action guard emits
// ACTION_CALLED_IN_OWNED_SCOPE and throws when an action is invoked under a
// children-capable owner. The violation invokes the action from a memo's compute
// on a re-run that a timer triggers; the control invokes it from an event
// handler, which has no owner.
import { action, createMemo, createSignal } from "solid-js";

const save = action(function* () {
  yield Promise.resolve();
});

export function Violation() {
  const [trigger, setTrigger] = createSignal(false);
  const label = createMemo(() => {
    if (trigger()) void save();
    return trigger();
  });
  setTimeout(() => setTrigger(true), 0);
  return <div>{String(label())}</div>;
}

export function Control() {
  return <button onClick={() => void save()}>save</button>;
}
