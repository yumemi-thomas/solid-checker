// The review of #217, A: `assigned = [cb].map(…)`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
let assigned: unknown;
setTimeout(() => {
  assigned = [cb].map((f) => runStep(f));
});

export function App() {
  return <p>x</p>;
}
