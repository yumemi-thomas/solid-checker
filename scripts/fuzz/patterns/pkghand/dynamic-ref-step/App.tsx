// The review of #217, D: `<Dynamic component="div" ref={cb} />` creates a `div` and applies the ref at once.
import { flush } from "solid-js";
import { runStep } from "fz-step";
import { Dynamic } from "@solidjs/web";
const cb = () => {
  flush();
};
function step() {
  const el = <Dynamic component="div" ref={cb} />;
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
