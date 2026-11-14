import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973), a guard for the census reason that is not there: a project wrapper hands its parameter to `createMemo`. The producers' hand-off (`handed_compute`,
// ADR 0873) follows a function literal and an arrow through such a wrapper, not a function DECLARATION handed by identifier (`work` below), so no start is found: the program looks
// computation-free, and SC1001's uncertifiable row (function-run-unplaced) is the only row at the read. Chrome rc.21 runs `work` as the computation and reports
// UNTRACKED_READ_AFTER_AWAIT there; the accounting keeps SC1002's uncertifiable row at the read.
function track<T>(compute: () => Promise<T>) {
  return createMemo(compute);
}
async function work() {
  await pause();
  return source();
}

createRoot(() => {
  track(work);
});

export function Probe() {
  return <p>probe</p>;
}
