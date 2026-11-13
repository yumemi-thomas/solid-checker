import { createMemo, createRoot, createSignal } from "solid-js";

const [count] = createSignal(0);
const pause = () => Promise.resolve();

// Batch 11, slice 9B (ADR 0973): there is no census reason. The program below holds no tracked async computation the producers find (no `createMemo(async ...)`, no effect
// compute that awaits), so no producer walks anything here, and every read written after an await still keeps its row: "the program holds no computation" is only as complete
// as the producers' index of computations, and Chrome shows starts the index does not find (the oracle cases `after-await-universe-census-*`).
async function load() {
  await pause();
  return count();
}
async function loadTwice() {
  await pause();
  count();
  return load();
}
void load();
void loadTwice();

export async function exported() {
  await pause();
  return count();
}

// The hazard in this fixture: `work` is a function declaration handed by identifier to a project wrapper that gives its parameter to `createMemo`. Chrome runs `work` as the
// computation and reports UNTRACKED_READ_AFTER_AWAIT at its read (the oracle case `after-await-universe-census-wrapper`); the producers' hand-off (ADR 0873) follows a function
// literal or an arrow through such a wrapper, not a declaration, so no start is found and the read is no one's but the accounting's.
function track(compute: () => Promise<number>) {
  return createMemo(compute);
}
async function work() {
  await pause();
  return count();
}
createRoot(() => {
  track(work);
});

// A call of a project function whose closure reads no source the checker lists keeps its row here too: the callee reason (ADR 0973) rests on the member-route net (ADR 0963), which runs
// only where a tracked async computation is recognized, and none is. `peek` reaches `holder.value`, a getter that reads `count`. Where the net is off the getter's read has no row of its
// own, and a start the producers' index does not find runs `viaCallee` as a computation (the oracle case `after-await-universe-census-callee`): Chrome reports the code at the getter's
// read, and this call is a frame of its stack.
const holder = {
  get value() {
    return count();
  },
};
function peek() {
  return holder.value;
}
export async function viaCallee() {
  await pause();
  return peek();
}
