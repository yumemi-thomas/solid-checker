// The review of #222 check 3, R1: a project wrapper reads its children in a step an unread package runs (from the wrapper's timer): the child expression is a getter of its props, run at the read.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
function helper() {
  flush();
  return "x";
}
function Wrap(props: { children?: unknown }) {
  setTimeout(() => {
    runStep(() => {
      void props.children;
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
