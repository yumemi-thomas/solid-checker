// The review of #217, A: `[cb].map(…)` in a template literal's substitution.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
setTimeout(() => {
  const text = `${[cb].map((f) => runStep(f)).length}`;
});

export function App() {
  return <p>x</p>;
}
