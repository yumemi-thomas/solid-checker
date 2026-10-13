import { createSignal } from "solid-js";
import { useLater, useMaybe, useTicker } from "dep-timer";

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
