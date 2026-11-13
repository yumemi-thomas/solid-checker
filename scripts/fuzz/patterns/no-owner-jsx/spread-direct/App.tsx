// The re-review of #222 follow-up: an intrinsic element's spread, created in an ownerless step a timer's callback calls: NO_OWNER_EFFECT in Chrome; the lane places it at the compiled `insert`/`spread` (chrome-host.mjs mapPositions).
const cb = () => 1;
function step() {
  const el = <div {...{ title: cb }} />;
}
setTimeout(() => {
  step();
});

export function App() {
  return <p>x</p>;
}
