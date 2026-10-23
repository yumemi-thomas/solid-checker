import { getOwner, onCleanup, runWithOwner } from "solid-js";

function handlers(cb: () => void) {
  return { onClick: cb };
}

// The mix of the root project, beside a synchronous dispatch the analysis
// cannot join to one element (a parameter receiver). It fires `ping` only, and
// the callback's event run ends at `onClick` (ADR 0361, amended): no dispatch
// runs it, so the event run is still a violation. `sync-dispatch-click` fires
// `click` and keeps the obligation.
export function RunWithOwnerAndHandler() {
  const owner = getOwner();
  const h = handlers(() => {
    onCleanup(() => undefined);
  });
  runWithOwner(owner, h.onClick);
  return <button {...h}>mix</button>;
}

export function fire(target: EventTarget) {
  target.dispatchEvent(new Event("ping"));
}
