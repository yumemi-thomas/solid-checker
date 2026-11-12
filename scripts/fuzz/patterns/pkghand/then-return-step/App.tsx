// The review of #217, C: a callback returns the value to the `.then` call it is an argument of.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
setTimeout(() => {
  void Promise.resolve()
    .then(() => cb)
    .then((f) => runStep(f));
});

export function App() {
  return <p>x</p>;
}
