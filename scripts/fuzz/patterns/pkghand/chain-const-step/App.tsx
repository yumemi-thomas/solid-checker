// The review of #222, B1: an assignment expression's own value is the value it assigns, so `const first = (last = cb)` places the callback in `first` too.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
let last: unknown;
setTimeout(() => {
  const first = (last = cb);
  runStep(first);
});

export function App() {
  return <p>x</p>;
}
