// The re-check of #226, N1 (props): a component calls a method of its props whose function reads a prop through `this` in a step.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
function helper() {
  flush();
  return "x";
}
function m(this: { children?: unknown }) {
  void runStep(() => {
    void this.children;
  });
}
function Wrap(props: { children?: unknown; pick: (this: { children?: unknown }) => void }) {
  setTimeout(() => {
    props.pick();
  });
  return null;
}
setTimeout(() => {
  document.body.append((<Wrap pick={m}>{helper()}</Wrap>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
