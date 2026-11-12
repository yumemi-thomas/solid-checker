import { flush } from "solid-js";
import { StepView } from "fz-step";
export function App() {
  setTimeout(() => {
    document.body.append((<StepView render={() => { flush(); }} />) as unknown as Node);
  });
  return <p>x</p>;
}
