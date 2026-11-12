// The re-review of #222 follow-up: a function child of an intrinsic element (an accessor), created in an ownerless step an unread package runs: NO_OWNER_EFFECT in Chrome; the lane places it at the compiled `insert`/`spread` (chrome-host.mjs mapPositions).
import { runStep } from "fz-step";
const cb = () => 1;
function step() {
  const el = <div>{cb as never}</div>;
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
