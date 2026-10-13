import { createSignal } from "solid-js";
import { choice, later, useLater, useMaybe, useTicker } from "dep-timer";
import { createRoot, getOwner } from "solid-js";

declare const flag: boolean;

// Violation at the call: an event handler has no owner, and with a numeric
// delay the package registers a cleanup through a helper on every call. The
// effect the accessor branch would create is unreachable from this call.
export function NumericInHandler() {
  return <button onClick={() => useTicker(() => {}, 1000)}>start</button>;
}

// Uncertifiable at the call: a signal argument proves no `typeof` from its
// syntax (ADR 0280), so neither branch is decided for this call.
export function AccessorInHandler() {
  const [delay] = createSignal(1000);
  return <button onClick={() => useTicker(() => {}, delay)}>start</button>;
}

// Clean: a component body has an owner.
export function InBody() {
  useTicker(() => {}, 1000);
  return <p>ticking</p>;
}

// Uncertifiable at the call: the cleanup is registered on some calls only.
export function MaybeInHandler() {
  return <button onClick={() => useMaybe(flag, () => {})}>maybe</button>;
}

// Not attributed: the package schedules the unowned callback itself.
export function LaterInHandler() {
  return <button onClick={() => useLater(() => {})}>later</button>;
}

// Violation at the call: a function literal decides `typeof delay`, so the
// effect is created on every such call and the cleanup branch never runs.
export function ArrowInHandler() {
  return <button onClick={() => useTicker(() => {}, () => 1000)}>start</button>;
}

// Clean: a member of the package function (`useTicker.length`, `.bind(…)`)
// does not run its body (ADR 0290 review round 1).
export function MemberInHandler() {
  const keep: Array<() => void> = [];
  return (
    <button onClick={() => keep.push(useTicker.bind(null, () => {}, 1000))}>
      {String(useTicker.length)}
    </button>
  );
}

// Clean: scheduling the package function (`setTimeout(useMaybe, …)`) is the
// reference's unowned run, not the direct call's; the direct call is owned.
export function ScheduledAndCalled() {
  useMaybe(true, () => {});
  setTimeout(useMaybe, 0, true, () => {});
  return <p>scheduled</p>;
}

// Uncertifiable, not a violation: `start` runs the package call only when
// `owned`, which only the root's call passes (review round 1, Sol).
function start(owned: boolean) {
  if (owned) useTicker(() => {}, 1000);
}
export function SharedHelper() {
  createRoot(() => start(true));
  return <button onClick={() => start(false)}>shared</button>;
}

// Uncertifiable, not a violation: the handler returns before the call when it
// has no owner.
export function OwnerGuard() {
  return (
    <button
      onClick={() => {
        if (!getOwner()) return;
        useTicker(() => {}, 1000);
      }}
    >
      guard
    </button>
  );
}

// Violation at the call: `choice(false)` runs the package's second cleanup
// site, though its first is unreachable for `false`.
export function SecondSite() {
  return <button onClick={() => choice(false)}>second</button>;
}

// Uncertifiable at the call: the package's cleanup after an `await`.
export function AfterAwait() {
  return <button onClick={() => void later()}>later</button>;
}
