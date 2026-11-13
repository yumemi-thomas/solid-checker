// The review of #222 check 3, R1: the same, the children destructured from the props in the step.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
function helper() {
  flush();
  return "x";
}
function Wrap(props: { children?: unknown }) {
  setTimeout(() => {
    runStep(() => {
      const { children } = props;
      void children;
    });
  });
  return null;
}
setTimeout(() => {
  document.body.append((<Wrap>{helper()}</Wrap>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
