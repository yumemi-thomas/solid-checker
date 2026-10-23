import { getOwner, onCleanup, runWithOwner } from "solid-js";

declare const ready: boolean;
declare function settle(): Promise<void>;

function handlers(cb: () => void) {
  return { onClick: cb };
}

function cleanup() {
  onCleanup(() => undefined);
}

// The cleanup follows a skippable `await`: after a suspension the owner is the
// original one or none (ADR 0357), so the event run is not proven ownerless
// and the cleanup stays an obligation.
export function AfterPossibleAwait() {
  const h = handlers(async () => {
    if (ready) await settle();
    onCleanup(() => undefined);
  });
  runWithOwner(getOwner(), h.onClick);
  return <button {...h}>await</button>;
}

// The same through a helper called after the skippable `await`: the call edge
// is the possible-suspension edge, which ends the event path.
export function HelperAfterPossibleAwait() {
  const h = handlers(async () => {
    if (ready) await settle();
    cleanup();
  });
  runWithOwner(getOwner(), h.onClick);
  return <button {...h}>helper</button>;
}
