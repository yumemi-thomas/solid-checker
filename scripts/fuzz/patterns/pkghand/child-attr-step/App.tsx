// The review of #222, D: the `children` attribute of an intrinsic element is inserted where the element is created.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function step() {
  const el = <div children={cb as never} />;
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
