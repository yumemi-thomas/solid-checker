// The re-review of #222, N1: a flush written in the package component's children expression runs when the component reads them.
import { flush } from "solid-js";
import { StepView } from "fz-step";
setTimeout(() => {
  document.body.append((<StepView>{(flush(), null) as never}</StepView>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
