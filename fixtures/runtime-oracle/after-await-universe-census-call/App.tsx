import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973), a guard for the census reason that is not there: a program that holds no tracked async computation the checker finds is no program without a
// computation. `createMemo.call(undefined, work)` starts one through a call no resolution of the primitive follows: Chrome rc.21 runs `work` as the computation and reports
// UNTRACKED_READ_AFTER_AWAIT at the read after the await, so a clean here is a false clean. The accounting keeps SC1002's uncertifiable row at the read.
async function work() {
  await pause();
  return source();
}

createRoot(() => {
  (createMemo as (compute: () => Promise<number>) => unknown).call(undefined, work);
});

export function Probe() {
  return <p>probe</p>;
}
