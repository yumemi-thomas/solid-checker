import { createSignal, onSettled } from "solid-js";

// W9's shape: evaluating `box.cb` runs a getter that throws, before the call that would read `s`. The read never runs.
export function App() {
  const [s] = createSignal(1);
  const f = () => {
    s();
  };
  const box = {
    get cb(): () => void {
      throw new Error("no callback");
    },
  };
  onSettled(box.cb);
  f();
  return <div />;
}
