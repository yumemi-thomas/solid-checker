// ADR 0506 (false positive): a binding initialized by a call holds its result; calling it does not run the arrow the wrapper never calls, so no write runs (the checker reported REACTIVE_WRITE_IN_OWNED_SCOPE).
import { createSignal } from "solid-js";

function wrapNever(fn: () => void): () => void {
  void fn;
  return () => {};
}

export function C1() {
  const [w1, setW1] = createSignal(0);
  const wrapped = wrapNever(() => setW1(7));
  wrapped();
  return (
    <div>{w1()}</div>
  );
}
