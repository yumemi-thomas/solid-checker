import { createSignal } from "solid-js";

// A binding initialized by a call holds the call's result, not the function
// handed to it: calling the binding is no call of that function (ADR 0506).

function wrapNever(fn: () => void): () => void {
  void fn;
  return () => {};
}

declare function wrapUnknown(fn: () => void): () => void;

function passThrough<T>(fn: T): T {
  return fn;
}

// Uncertifiable (was a violation): `wrapNever` never runs the arrow; calling
// its result runs a different function.
export function NeverRuns() {
  const [count, setCount] = createSignal(0);
  const wrapped = wrapNever(() => setCount(1));
  wrapped();
  return <p>{count()}</p>;
}

// Uncertifiable (was a violation): what an undeclared-source wrapper returns
// is unknown.
export function UnknownWrapper() {
  const [count, setCount] = createSignal(0);
  const wrapped = wrapUnknown(() => setCount(2));
  wrapped();
  return <p>{count()}</p>;
}

// Violation: an identity wrapper returns its argument (ADR 0346), so the call
// runs the arrow in the body.
export function IdentityWrapper() {
  const [count, setCount] = createSignal(0);
  const wrapped = passThrough(() => setCount(3));
  wrapped();
  return <p>{count()}</p>;
}

// Violation (control): a binding initialized by the arrow itself.
export function DirectBinding() {
  const [count, setCount] = createSignal(0);
  const direct = () => setCount(4);
  direct();
  return <p>{count()}</p>;
}
