import { createRoot, onCleanup } from "solid-js";

const noop = () => {};

function register(fn: () => void) {
  onCleanup(fn);
}

function registerDirect(fn: () => void) {
  onCleanup(fn);
}

function registerCalled(fn: () => void) {
  onCleanup(fn);
}

// Clean: `register.bind(…)` reads a member of `register` and does not run
// it. The bound function runs under createRoot's owner, or never.
export function Bound() {
  const keep: Array<() => void> = [];
  return (
    <button
      onClick={() => {
        createRoot(register.bind(null, noop));
        keep.push(register.bind(null, noop));
      }}
    >
      bind
    </button>
  );
}

// Violation: a direct call in an event handler runs it with no owner.
export function Direct() {
  return <button onClick={() => registerDirect(noop)}>direct</button>;
}

// Violation: `call` runs its receiver before it returns.
export function Called() {
  return <button onClick={() => registerCalled.call(null, noop)}>call</button>;
}
