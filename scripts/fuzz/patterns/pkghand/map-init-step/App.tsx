// The review of #217, A: `[cb].map(…)` as a binding's initializer: the literal is the receiver of the call, the call's value another one.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
setTimeout(() => {
  const ps = [cb].map((f) => runStep(f));
});

export function App() {
  return <p>x</p>;
}
