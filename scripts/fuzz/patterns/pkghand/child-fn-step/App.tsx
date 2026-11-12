// The review of #222, D: a function child of an intrinsic element is called as an accessor where the element is created.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function step() {
  const el = <div>{cb as never}</div>;
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
