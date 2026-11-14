import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973), a guard for the census reason that is not there: `Reflect.apply(createMemo, undefined, [work])` starts a tracked async computation through a call
// no resolution of the primitive follows (the callee is `Reflect.apply`). Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read after the await in `work`; the read keeps
// SC1002's uncertifiable row.
async function work() {
  await pause();
  return source();
}

createRoot(() => {
  Reflect.apply(createMemo as (compute: () => Promise<number>) => unknown, undefined, [work]);
});

export function Probe() {
  return <p>probe</p>;
}
