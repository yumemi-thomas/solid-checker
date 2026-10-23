import { getOwner, onCleanup, runWithOwner } from "solid-js";

function handlers(cb: () => void) {
  return { onClick: cb };
}

// The mix of the root project, beside a synchronous dispatch of `click` the
// analysis cannot join to one element (a parameter receiver): the click
// handler may run on the dispatcher's stack, under its owner (ADR 0356, 0361),
// so the event run is not proven ownerless and the callback keeps its
// obligation.
export function RunWithOwnerAndHandler() {
  const owner = getOwner();
  const h = handlers(() => {
    onCleanup(() => undefined);
  });
  runWithOwner(owner, h.onClick);
  return <button {...h}>mix</button>;
}

export function fire(target: EventTarget) {
  target.dispatchEvent(new Event("click"));
}
