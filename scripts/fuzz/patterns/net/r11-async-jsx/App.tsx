import { createMemo } from "solid-js";
const asy = createMemo(async () => 1);
function run(o: { go: () => number }) { return o.go(); }
export function App() {
  return <p>{run({ go: asy })}</p>;
}
