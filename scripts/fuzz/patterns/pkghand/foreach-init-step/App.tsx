// The review of #217, A: `[cb].forEach(…)` as a binding's initializer.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
setTimeout(() => {
  const n = [cb].forEach((f) => {
    runStep(f);
  });
});

export function App() {
  return <p>x</p>;
}
