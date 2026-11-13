import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973), a guard for the census reason that is not there: `[createMemo][0](work)` calls the primitive through an array element, a call whose callee is no
// reference the primitive resolution follows. Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read after the await in `work`; the read keeps SC1002's uncertifiable row.
async function work() {
  await pause();
  return source();
}

createRoot(() => {
  const primitives = [createMemo as (compute: () => Promise<number>) => unknown];
  primitives[0](work);
});

export function Probe() {
  return <p>probe</p>;
}
