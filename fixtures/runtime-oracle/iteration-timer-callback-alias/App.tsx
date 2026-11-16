import { createSignal, createStore } from "solid-js";

// ADR 1013: a timer handed a `const` that names an arrow calls that arrow, which has no `this`: the window reaches no code, a primitive's array pattern stays
// exact, and the writes below are the defects they look like. Chrome rc.21 reports both; the checker proves both (it withdrew the slots before ADR 1013).
const run = () => {};
const again = run;
setTimeout(again, 0);

// REACTIVE_WRITE_IN_OWNED_SCOPE: a setter called in a component body.
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}

// STORE_WRITE_OUTSIDE_SETTER: the fuzz grid's lost shape (`mx-store-write-outside-setter-timer-…-cb-alias`): the write runs from a timer handed an alias.
export function TimerWrite() {
  const [s1] = createStore({ v: 0 });
  const cb2 = () => {
    s1.v = 7;
  };
  const cb1 = cb2;
  setTimeout(cb1, 0);
  return <div>{s1.v}</div>;
}
