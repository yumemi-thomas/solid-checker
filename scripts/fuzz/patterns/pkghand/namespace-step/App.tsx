import { runStepNs } from "fz-step";
import * as NS from "./nsmod";
export function App() {
  setTimeout(() => {
    runStepNs(NS);
  });
  return <p>x</p>;
}
