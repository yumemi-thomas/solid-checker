// The review of #217, B: a project function that names `arguments` reads every argument through it.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function w2(_f: () => void) {
  runStep(arguments[0]);
}
setTimeout(() => {
  w2(cb);
});

export function App() {
  return <p>x</p>;
}
