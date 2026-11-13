// The review of #217, A: `yield cb` in a generator and `for (const f of gen())`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function* generate(): Generator<() => void, void, undefined> {
  const x = yield cb;
}
setTimeout(() => {
  for (const f of generate()) runStep(f);
});

export function App() {
  return <p>x</p>;
}
