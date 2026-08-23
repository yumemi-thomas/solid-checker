import { createState, drop } from "reactive-package";

const state = createState();
declare const opaque: number[];

function wrappedDrop<T>(values: T[], count = 1) {
  return drop(values, count);
}

export function ReactiveArgument() {
  const first = wrappedDrop(state, 1)[0];
  return <div>{first}</div>;
}

export function PlainArgument() {
  const first = wrappedDrop([1, 2, 3], 1)[0];
  return <div>{first}</div>;
}

export function UnknownArgument() {
  const first = wrappedDrop(opaque, 1)[0];
  return <div>{first}</div>;
}

// Spreading the store copies out of the proxy here, so the callee receives
// snapshot data and its parameter-member read proves nothing. The reactive
// read is the spread, and it is reported as such — once, at the spread.
export function SpreadArgument() {
  const first = wrappedDrop([...state], 1)[0];
  return <div>{first}</div>;
}

// The same literal built from plain data reads nothing at all.
export function SpreadOfLiteralArgument() {
  const first = wrappedDrop([...[1, 2, 3]], 1)[0];
  return <div>{first}</div>;
}
