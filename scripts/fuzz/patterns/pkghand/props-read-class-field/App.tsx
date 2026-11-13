// The review of #226, B2: a project wrapper reads its children in a class field initializer, run when the class is made in a step.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
function helper() {
  flush();
  return "x";
}
function Wrap(props: { children?: unknown }) {
  class K {
    v = props.children === undefined;
  }
  setTimeout(() => {
    void runStep(() => {
      void new K();
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
