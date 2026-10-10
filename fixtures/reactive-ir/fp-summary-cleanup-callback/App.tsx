import { createSignal, onCleanup } from "solid-js";

// ---- Negatives: an `onCleanup` callback runs when the owner is disposed or
// re-run, never inside the strict-read window of the body that registered it.

export function CleanupLiteral() {
  const [n] = createSignal(0);
  onCleanup(() => {
    if (n()) console.log("bye");
  });
  return <div />;
}

export function CleanupExpression() {
  const [n] = createSignal(0);
  onCleanup(() => console.log(n()));
  return <div />;
}

// Through a derived helper the callback calls.
export function CleanupThroughHelper() {
  const [n] = createSignal(0);
  const label = () => `n=${n()}`;
  onCleanup(() => console.log(label()));
  return <div />;
}

// A named callback handed to `onCleanup`.
export function CleanupNamed() {
  const [n] = createSignal(0);
  const stop = () => console.log(n());
  onCleanup(stop);
  return <div />;
}

// ---- Positive controls: reads in the body itself, around the registration.

export function BodyRead() {
  const [n] = createSignal(0);
  onCleanup(() => console.log("bye"));
  const snapshot = n();
  return <span>{snapshot}</span>;
}

// The read feeding the cleanup callback happens in the body.
export function ReadBeforeCleanup() {
  const [n] = createSignal(0);
  const seen = n();
  onCleanup(() => console.log(seen));
  return <div />;
}
