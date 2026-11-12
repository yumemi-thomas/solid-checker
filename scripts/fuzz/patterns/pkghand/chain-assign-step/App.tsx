// The review of #222, B1: a chained assignment, `first = last = cb`, writes the callback to both targets.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
let first: () => void = () => {};
let last: unknown;
setTimeout(() => {
  first = last = cb;
  runStep(first);
});

export function App() {
  return <p>x</p>;
}
