import { createEffect, createMemo } from "solid-js";
import { render } from "@solidjs/web";

// The handler of `Handler` runs on the browser's own stack, so it is never on an effect's apply (`EffectCreationRunsOffApply`), unless the program dispatches an event
// synchronously: a DOM dispatch runs the handlers on its caller's stack. `Clicker` calls `click()` in an effect's apply, so the handler's creation is no longer placed off every
// apply: it keeps its uncertifiable row (`effect-creation-unaccounted`).
export function Handler() {
  return (
    <button
      onClick={() => {
        createMemo(() => 1);
      }}
    >
      x
    </button>
  );
}

export function Clicker() {
  createEffect(
    () => 0,
    () => {
      document.body.click();
    },
  );
  return <p>clicker</p>;
}

render(
  () => (
    <p>
      <Handler />
      <Clicker />
    </p>
  ),
  document.body,
);
