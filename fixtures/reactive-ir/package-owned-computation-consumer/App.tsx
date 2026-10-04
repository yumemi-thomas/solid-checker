import { createSignal } from "solid-js";
import { derive, deriveMaybe, deriveWrapped, watch } from "reactive-package";

// `derive` runs its argument during every call as the compute of a memo it
// creates: the write throws REACTIVE_WRITE_IN_OWNED_SCOPE under that memo.
export function WriteInDerive() {
  const [count, setCount] = createSignal(0);
  derive(() => setCount(1));
  return <div>{count()}</div>;
}

// The same compute run from an event handler throws too, but a write in a
// function nested in a JSX attribute's function is not classified: a
// conservative miss, not a claim.
export function WriteInDeriveFromHandler() {
  const [count, setCount] = createSignal(0);
  return <div onClick={() => derive(() => setCount(2))}>{count()}</div>;
}

// `watch`'s first argument is an effect compute, run during the call.
export function WriteInWatchCompute() {
  const [count, setCount] = createSignal(0);
  watch(
    () => {
      setCount(3);
      return 0;
    },
    () => {}
  );
  return <div>{count()}</div>;
}

// The effect function is queued and runs where writes are legal.
export function WriteInWatchEffect() {
  const [count, setCount] = createSignal(0);
  watch(
    () => count(),
    () => setCount(4)
  );
  return <div>{count()}</div>;
}

// `deriveMaybe` may never run its argument: no proven violation.
export function WriteInDeriveMaybe(props: { on: boolean }) {
  const [count, setCount] = createSignal(0);
  deriveMaybe(props.on, () => setCount(5));
  return <div>{count()}</div>;
}

// `deriveWrapped` states no owner for its callback: no proven violation.
export function WriteInDeriveWrapped() {
  const [count, setCount] = createSignal(0);
  deriveWrapped(() => setCount(6));
  return <div>{count()}</div>;
}

// A closure the compute only returns does not run in the compute.
export function WriteInReturnedClosure() {
  const [count, setCount] = createSignal(0);
  derive(() => () => setCount(7));
  return <div>{count()}</div>;
}
