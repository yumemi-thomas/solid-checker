import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const [state, setState] = createSignal(0);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B follow-up (ADR 0976), S2 and R1 to R4 of the reviews of #233: a call into the standard library, or a setter, that runs a project function on the
// continuation's stack. Each left the accounting pass through the arm that skipped such calls, and Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT under each. The functions are
// declarations handed by identifier to a project wrapper that gives its parameter to `createMemo` (a start the producers' index does not follow, so no walk reaches them), and each
// call is written on one line (the judge anchors a finding at its start line, and a listener runs on a line of its own: the call's frame is the one a row at the call covers).
function reader() {
  return source();
}
function track(compute: () => Promise<unknown>) {
  return createMemo(compute);
}

// R2: listeners registered earlier, which a synchronous dispatch delivers to on the caller's stack.
const target = new EventTarget();
target.addEventListener("x", () => {
  source();
});
const button = document.createElement("button");
button.addEventListener("click", () => {
  source();
});
const controller = new AbortController();
controller.signal.addEventListener("abort", () => {
  source();
});
// R3: a stored generator, stepped by a call with an argument. R4: an iterator literal and a `Map` subclass under their library types.
function* numbers(): Generator<number, void, number> {
  yield source();
}
const stored = numbers();
const iterator: Iterator<number> = {
  next() {
    source();
    return { done: true, value: 1 };
  },
};
class Counts extends Map<string, number> {
  get(key: string) {
    source();
    return super.get(key);
  }
}
const counts: Map<string, number> = new Counts();

async function viaUpdater() {
  await pause();
  return setState(() => source());
}
async function viaSetterRef() {
  await pause();
  return setState(reader);
}
async function viaDispatch() {
  await pause();
  return target.dispatchEvent(new Event("x"));
}
async function viaClick() {
  await pause();
  return button.click();
}
async function viaAbort() {
  await pause();
  return controller.abort();
}
async function viaNext() {
  await pause();
  return stored.next(1).value;
}
async function viaIterator() {
  await pause();
  return iterator.next(undefined);
}
async function viaMap() {
  await pause();
  return counts.get("x");
}
async function viaForEach() {
  await pause();
  return [1].forEach(() => { source(); });
}
async function viaFrom() {
  await pause();
  return Array.from([1], reader);
}
async function viaExecutor() {
  await pause();
  return new Promise<number>((done) => done(source()));
}

createRoot(() => {
  track(viaUpdater);
  track(viaSetterRef);
  track(viaDispatch);
  track(viaClick);
  track(viaAbort);
  track(viaNext);
  track(viaIterator);
  track(viaMap);
  track(viaForEach);
  track(viaFrom);
  track(viaExecutor);
});
void state;

export function Probe() {
  return <p>probe</p>;
}
