// SC2006 flush-in-action. The dev build's flush guard throws "[FLUSH_IN_ACTION]"
// while an action step runs. The violation starts an action from a timer; its
// first step calls flush() before the first yield, and the action's rejected
// promise is left unhandled. The control drains from an event handler, outside
// any action.
import { action, flush } from "solid-js";

const save = action(function* () {
  flush();
  yield Promise.resolve();
});

export function Violation() {
  setTimeout(() => void save(), 0);
  return <div />;
}

export function Control() {
  return <button onClick={() => flush()}>flush</button>;
}
