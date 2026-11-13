// The review of #222 check 3, R1: the same with a map child.
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
  document.body.append((<Wrap>{[1].map(() => helper())}</Wrap>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
