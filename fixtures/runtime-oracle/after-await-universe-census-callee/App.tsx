import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973), a guard for the callee reason's premise: the callee summary (a call whose callee reads nothing the checker lists ends its site) rests on the member-route
// net (ADR 0963), which runs only where a tracked async computation is recognized. Here a project wrapper hands `work`, a function DECLARATION, to `createMemo` (a start the producers'
// index does not find), `work` calls `peek()` after its await, and `peek` reaches a getter that reads `source`: Chrome rc.21 raises UNTRACKED_READ_AFTER_AWAIT at the getter's read, and
// the call `peek()` is a frame of its stack. No computation is recognized, so the net is off and the getter's read has no row of its own: the call keeps its row, which covers it.
const holder = {
  get value() {
    return source();
  },
};
function peek() {
  return holder.value;
}
function track(compute: () => Promise<number>) {
  return createMemo(compute);
}
async function work() {
  await pause();
  return peek();
}

createRoot(() => {
  track(work);
});

export function Probe() {
  return <p>probe</p>;
}
