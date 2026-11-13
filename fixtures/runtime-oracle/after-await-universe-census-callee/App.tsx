import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973), a guard for the call that keeps its row where the member-route net (ADR 0963) is off. A project wrapper hands `work`, a function DECLARATION, to
// `createMemo` (a start the producers' index does not find), `work` calls `peek()` after its await, and `peek` reaches a getter that reads `source`: Chrome rc.21 raises
// UNTRACKED_READ_AFTER_AWAIT at the getter's read, and the call `peek()` is a frame of its stack. No computation is recognized, so the net is off and the getter's read has no row of
// its own: the call keeps its row, which covers it. (The first build ended `peek()` with a callee reason, which this case caught; there is none now, the review of #233's B1.)
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
