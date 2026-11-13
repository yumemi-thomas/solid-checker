import { createMemo } from "solid-js";
import { runStep } from "fz-step";
export function App() {
  const m = createMemo(() => {
    void runStep(() => {});
    return 1;
  });
  return <p>{m()}</p>;
}
