import { getOwner, onCleanup, runWithOwner } from "solid-js";

function handlers(cb: () => void) {
  return { onClick: cb };
}

// A `window.dispatchEvent` reaches no element handler (ADR 0361), so it
// withholds nothing: the event run of the mixed callback is still a violation.
export function RunWithOwnerAndHandler() {
  const owner = getOwner();
  const h = handlers(() => {
    onCleanup(() => undefined);
  });
  runWithOwner(owner, h.onClick);
  window.dispatchEvent(new CustomEvent("ping"));
  return <button {...h}>mix</button>;
}
