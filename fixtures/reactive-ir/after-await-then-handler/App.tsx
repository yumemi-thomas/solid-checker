import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B follow-up (ADR 0976), S3 of the review of #233: the continuation of a promise chain in a function that is no `async` function. `function work() { return
// pause().then(() => source()); }` is the compute of `createMemo` once a project wrapper hands it on (a start the producers' index does not follow, and no walk follows a chain from
// it): the compute returns the chained promise, and Chrome rc.21 raises UNTRACKED_READ_AFTER_AWAIT at the read in the handler. A promise handler is code written after a suspension:
// `then`, `catch`, `finally`, and a handler handed by reference. `viaPure` is the control: its handler reads nothing and calls no project function, so nothing is listed. Each
// handler is written on one line.
function reader() {
  return source();
}
function track(compute: () => Promise<unknown>) {
  return createMemo(compute);
}
function viaThen() {
  return pause().then(() => source());
}
function viaCatch() {
  return Promise.reject(new Error("no")).catch(() => source());
}
function viaFinally() {
  return pause().finally(() => { source(); });
}
function viaReference() {
  return pause().then(reader);
}
function viaPure() {
  return pause().then(() => 1);
}
function viaComputed() {
  return pause()["then"](() => source());
}
// No computation starts this function, so no walk follows its chain; the handler is after a suspension all the same (there is no census reason): uncertifiable, never silent.
export function standalone() {
  return pause().then(() => source());
}

createRoot(() => {
  track(viaThen);
  track(viaCatch);
  track(viaFinally);
  track(viaReference);
  track(viaPure);
  track(viaComputed);
});

export function Probe() {
  return <div>probe</div>;
}
