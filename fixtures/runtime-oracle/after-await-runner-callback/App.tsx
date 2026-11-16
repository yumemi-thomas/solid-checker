import { createMemo, createSignal, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// ADR 1004: the stack semantics the `runner-scheduled` reason rests on. rc.21 raises UNTRACKED_READ_AFTER_AWAIT only for a read with an async computation's
// flight on its async stack. `Scheduled` registers async callbacks with a loop written like a test runner's (the callbacks are recorded, then called and
// awaited one by one from a task of the loop's own, vitest's `await fn()` in `runTest`): each reads after its own await, and calls a function that reads, and the
// runtime is silent at every one of those reads. `OnFlight` is the twin: the same body awaited by an async memo's compute, where the runtime raises the code at
// the read. The checker's rows here are uncertifiable on both sides (the loop is the project's own, no reviewed runner), so the case pins what Chrome does, not a
// silence of the checker; the fixture `fixtures/reactive-ir/after-await-runner-callback` pins the checker on a reviewed runner.
const [fresh] = createSignal(1);
const [flown] = createSignal(1);
const readsFresh = () => fresh();

type Registered = () => Promise<void>;
const registered: Registered[] = [];
const register = (fn: Registered) => {
  registered.push(fn);
};
async function runLoop() {
  for (const fn of registered) {
    await fn();
  }
}

export function Scheduled() {
  register(async () => {
    await pause();
    fresh();
    readsFresh();
  });
  register(async () => {
    await pause();
    await pause();
    fresh();
  });
  setTimeout(() => void runLoop(), 0);
  return <span>scheduled</span>;
}

async function flownBody() {
  await pause();
  flown();
}

export function OnFlight() {
  const data = createMemo(async () => {
    await flownBody();
    return 1;
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}
