// The review of #222, B1: an assignment written as an element of a literal that is a receiver, `[(last = cb)].map(…)`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
let last: unknown;
setTimeout(() => {
  const rs = [(last = cb)].map((f) => runStep(f));
});

export function App() {
  return <p>x</p>;
}
