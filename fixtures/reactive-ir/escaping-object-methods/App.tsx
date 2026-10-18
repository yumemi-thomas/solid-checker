import { createSignal } from "solid-js";

// An object literal's shorthand method that leaves through anything other
// than a resolved member call is a stored callback: whoever holds the object
// may call it, during the body or later.

export function HandedToUnknown() {
  const [count] = createSignal(0);
  opaqueFn({
    run() {
      console.log(count());
    },
  });
  return <main />;
}
export function HeldThenHanded() {
  const [count] = createSignal(0);
  const held = {
    run() {
      console.log(count());
    },
  };
  opaqueFn(held);
  return <main />;
}
export function Enumerated() {
  const [count] = createSignal(0);
  Object.values({
    run() {
      console.log(count());
    },
  }).forEach((run) => run());
  return <main />;
}

// Control: the literal is only ever the receiver of a member call, which the
// call site resolves (here, a proven read in the body).
export function DispatchedOnly() {
  const [count] = createSignal(0);
  const held = {
    run() {
      console.log(count());
    },
  };
  held.run();
  return <main />;
}
