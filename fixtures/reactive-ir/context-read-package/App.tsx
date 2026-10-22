import { useDialogContext } from "dep-dialog-context";
import { useThing } from "dep-unselected";

// Violation at the project's call: the package hook reads its context with no
// owner, because an event handler has none. The read is in the package's
// source; the finding is placed at the call that runs it (ADR 0290).
export function HandlerHook() {
  return <button onClick={() => useDialogContext()}>open</button>;
}

// Negative: the component body has an owner.
export function BodyHook() {
  useDialogContext();
  return <p>open</p>;
}

// Uncertifiable: the hook runs only when `armed` is true, which the source does
// not prove on every invocation of the handler.
export function ConditionalHook(props: { armed: boolean }) {
  return (
    <button
      onClick={() => {
        if (props.armed) useDialogContext();
      }}
    >
      maybe
    </button>
  );
}

// Uncertifiable: `dep-unselected` is installed, but its source is not selected
// for analysis, so what the hook does is unavailable (SC9005), not clean.
export function UnselectedHook() {
  return <button onClick={() => useThing()}>unselected</button>;
}
