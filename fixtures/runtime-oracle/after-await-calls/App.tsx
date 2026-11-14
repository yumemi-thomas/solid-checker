import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973), the review of #233's B2 and B3: calls the resolver names no project function, each of which runs one on the continuation's stack. They left the
// accounting pass through a `continue`, and Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT under each. The functions are declarations handed by identifier to a project wrapper
// that gives its parameter to `createMemo` (a start the producers' index does not follow, so no walk reaches them): a method of a module-scope `const` object literal the receiver
// census withholds, its computed and optional members and an arrow property (B2); `f.call`, `f.apply`, `Reflect.apply` and a function literal called where it is written (B3).
// The function expression is written on one line: the judge anchors a finding at its start line, and Chrome's frame for a multi-line immediately invoked function is its closing line
// (an attribution question for the judge, `docs/open-work.md`, like `async-iife-L`; the fixture `after-await-universe` holds the multi-line spelling, covered by its row).
const registry = {
  run() {
    return source();
  },
  tick: () => source(),
};
function reader() {
  return source();
}
function track(compute: () => Promise<unknown>) {
  return createMemo(compute);
}
async function viaMethod() {
  await pause();
  return registry.run();
}
async function viaComputed() {
  await pause();
  return registry["run"]();
}
async function viaOptional() {
  await pause();
  return registry?.run();
}
async function viaArrow() {
  await pause();
  return registry.tick();
}
async function viaCall() {
  await pause();
  return reader.call(undefined);
}
async function viaApply() {
  await pause();
  return reader.apply(undefined, []);
}
async function viaReflect() {
  await pause();
  return Reflect.apply(reader, undefined, []);
}
async function viaIife() {
  await pause();
  return (() => source())();
}
async function viaFunctionIife() {
  await pause();
  return (function () { return source(); })();
}
async function viaAsyncIife() {
  await pause();
  return await (async () => source())();
}

createRoot(() => {
  track(viaMethod);
  track(viaComputed);
  track(viaOptional);
  track(viaArrow);
  track(viaCall);
  track(viaApply);
  track(viaReflect);
  track(viaIife);
  track(viaFunctionIife);
  track(viaAsyncIife);
});

export function Probe() {
  return <p>probe</p>;
}
