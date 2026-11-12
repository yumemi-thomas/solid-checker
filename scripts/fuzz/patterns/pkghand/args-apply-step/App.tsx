// The review of #217, B: `takesRest.apply(null, arguments)` forwards every argument.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function takesRest(...fs: Array<() => void>) {
  runStep(fs[0]);
}
function w3(_f: () => void) {
  takesRest.apply(null, arguments as unknown as Array<() => void>);
}
setTimeout(() => {
  w3(cb);
});

export function App() {
  return <p>x</p>;
}
