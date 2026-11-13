// The check of #227 (the neighbour of G1, no prototype): a method of the literal called through `this` in a closure a package runs. No call edge reaches `m`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o = {
  m(): void {
    flush();
  },
  run(): void {
    void runStep(() => {
      this.m();
    });
  },
};
setTimeout(() => {
  o.run();
});

export function App() {
  return <p>x</p>;
}
