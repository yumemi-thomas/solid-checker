// The re-review of #222, N1: an element in a callback of an inline call in the package component's children is created when the component reads them.
import { flush } from "solid-js";
import { StepView } from "fz-step";
function Own() {
  flush();
  return null;
}
setTimeout(() => {
  document.body.append((<StepView>{[1].map(() => <Own />) as never}</StepView>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
