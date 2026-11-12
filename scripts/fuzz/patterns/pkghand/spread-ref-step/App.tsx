// The review of #222, D: a `ref` that reaches an intrinsic element through a spread of an object literal.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function step() {
  const el = <div {...{ ref: cb }} />;
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
