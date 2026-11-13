// The review of #222 check 3, R2: an object literal's getter, read through a parameter in a step an unread package runs, runs at the read.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const o = {
  get g() {
    flush();
    return 1;
  },
};
function w(p: { g: number }) {
  runStep(() => {
    void p.g;
  });
}
setTimeout(() => {
  w(o);
});

export function App() {
  return <p>x</p>;
}
