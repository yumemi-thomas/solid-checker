// The review of #217, A: `await [cb].map(…)[0]`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
setTimeout(async () => {
  await [cb].map((f) => runStep(f))[0];
});

export function App() {
  return <p>x</p>;
}
