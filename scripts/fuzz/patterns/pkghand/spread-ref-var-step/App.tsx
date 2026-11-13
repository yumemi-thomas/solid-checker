// The review of #222, D: a `ref` that reaches an intrinsic element through a spread of a binding.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
const props = { ref: cb };
function step() {
  const el = <div {...props} />;
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
