import { onCleanup } from "solid-js";

// ADR 0970, B3 of the review of #221: the program of fixtures/reactive-ir/owner-emission-eval. A direct `eval` reaches any binding by name: the timer's `eval("Evaled()")` calls the
// component with no owner, so Chrome rc.21 reports NO_OWNER_CLEANUP at its `onCleanup`. The owner emission certified it (it asked for `eval` after the stop that closes a component
// rendered by tags alone).
function Evaled() {
  onCleanup(() => {});
  return <p>evaled</p>;
}
export function Host() {
  setTimeout(() => {
    eval("Evaled()");
  }, 0);
  return (
    <p>
      <Evaled />
    </p>
  );
}
