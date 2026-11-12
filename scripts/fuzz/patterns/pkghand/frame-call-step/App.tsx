// The re-review of #222, N1: a call in the package component's children runs when the component reads them.
import { flush } from "solid-js";
import { StepView } from "fz-step";
function helper() {
  flush();
  return null;
}
setTimeout(() => {
  document.body.append((<StepView>{helper() as never}</StepView>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
