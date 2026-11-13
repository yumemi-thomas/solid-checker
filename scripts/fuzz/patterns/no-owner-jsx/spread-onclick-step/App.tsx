// The re-review of #222 follow-up: an intrinsic element's spread, created in an ownerless step an unread package runs: NO_OWNER_EFFECT in Chrome; the lane places it at the compiled `insert`/`spread` (chrome-host.mjs mapPositions).
import { runStep } from "fz-step";
const cb = () => 1;
function step() {
  const el = <div {...{ onClick: cb }} />;
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
