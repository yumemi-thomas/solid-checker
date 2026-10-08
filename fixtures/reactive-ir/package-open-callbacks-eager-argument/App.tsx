import { createSignal } from "solid-js";
import { withDefaults, withOverrides } from "reactive-package";

// The read is an argument: the caller evaluates it now, in the component body,
// whatever the export later does with the value. The open `callbacks` claim is
// about functions the export receives, and `overlay()` is not one.
export function EagerRead() {
  const [overlay] = createSignal({ label: "x" });
  withDefaults({}, overlay());
  return <div />;
}

// A spread is evaluated before the call just the same.
export function EagerSpread() {
  const [overlays] = createSignal<object[]>([]);
  withDefaults({}, ...overlays());
  return <div />;
}

// Argument 0 of the other export: the fix is not tied to one position.
export function EagerFirst() {
  const [overlay] = createSignal({ label: "x" });
  withOverrides(overlay(), {});
  return <div />;
}

// A function literal handed over is the export's to run, at a time the
// contract does not state: uncertifiable, never a proven violation.
export function DeliveredCallback() {
  const [overlay] = createSignal({ label: "x" });
  withDefaults({}, () => overlay());
  return <div />;
}

// The same literal behind a transparent TypeScript wrapper.
export function WrappedCallback() {
  const [overlay] = createSignal({ label: "x" });
  withDefaults({}, (() => overlay()) as () => object);
  return <div />;
}

// A function selected inside an eager expression is still a delivered
// function: its body keeps the unknown timing.
export function SelectedCallback() {
  const on = Math.random() > 0.5;
  const [overlay] = createSignal({ label: "x" });
  withDefaults({}, on ? () => overlay() : {});
  return <div />;
}
