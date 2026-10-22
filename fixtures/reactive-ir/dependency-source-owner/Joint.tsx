import { useListener, useListenerAsync, useMixed, useSkippable, useTried } from "dep-timer";

// Violation at the call: both branches of `useListener` create an owned effect
// on every exit, so the handler's unowned call runs one of them unowned.
export function ListenerInHandler() {
  return <button onClick={() => useListener(1000, () => {})}>listen</button>;
}

// Uncertifiable at the call: the early return skips both branches on some calls.
export function SkippableInHandler() {
  return <button onClick={() => useSkippable(1000, () => {})}>skippable</button>;
}

// Uncertifiable at the call: an async body is not modeled as a cover.
export function AsyncListenerInHandler() {
  return <button onClick={() => useListenerAsync(1000, () => {})}>async</button>;
}

// Uncertifiable at the call: the branches sit in a `try`.
export function TriedInHandler() {
  return <button onClick={() => useTried(1000, () => {})}>tried</button>;
}

// Uncertifiable at the call: an effect and a cleanup are different operations.
export function MixedInHandler() {
  return <button onClick={() => useMixed(1000, () => {})}>mixed</button>;
}

// Clean: a component body has an owner.
export function ListenerInBody() {
  useListener(1000, () => {});
  return <p>listening</p>;
}
