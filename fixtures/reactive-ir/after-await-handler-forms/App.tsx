import { createMemo, createRoot, createSignal } from "solid-js";

const [source] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// ADR 1003: a promise handler that is no function literal and no function name is followed to the functions it may be. Chrome rc.21 raises
// UNTRACKED_READ_AFTER_AWAIT at the read in each of them (`fixtures/runtime-oracle/after-await-handler-forms`); ADR 0976 listed only a literal and an exact name.
// Where the walk cannot name every function, the handler argument itself is the site. Each read is written on one line.
function reader() {
  return source();
}
function other() {
  return source() + 1;
}
const handlers = { ok: () => source() };
function track(compute: () => Promise<unknown>) {
  return createMemo(compute);
}
// U at the read in `handlers.ok`, and U at the argument: nothing proves the object kept the function.
function viaMember() {
  return pause().then(handlers.ok);
}
// U at the read in `other` and in `reader`: both operands may be the handler.
function viaConditional() {
  return pause().then(Math.random() > 2 ? other : reader);
}
// U at the read in the function the caller hands, and U at the argument in `chain`: a caller the program does not name may hand any function.
function chain(handler: () => number) {
  return pause().then(handler);
}
function viaParameter() {
  return chain(() => source());
}
// U at the read in `bound`: `f.bind(…)` runs `f`.
function bound() {
  return source() + 2;
}
function viaBound() {
  return pause().then(bound.bind(null));
}
// U at the read in the element of the array that is spread.
function viaSpread() {
  const fns = [() => source()] as const;
  return pause().then(...fns);
}
// U at the argument: a call's result is a function nothing names.
function make(): () => number {
  return () => 1;
}
function viaResult() {
  return pause().then(make());
}
// no row: the parameters of the executor of a standard `new Promise` are the library's `resolve` and `reject`, and a nullish literal is no function.
function viaExecutor() {
  return new Promise<void>((resolve, reject) => { pause().then(resolve, reject); });
}
function viaNullish() {
  return pause().then(undefined, null);
}

createRoot(() => {
  track(viaMember);
  track(viaConditional);
  track(viaParameter);
  track(viaBound);
  track(viaSpread);
  track(viaResult);
  track(viaExecutor);
  track(viaNullish);
});

export function Probe() {
  return <p>probe</p>;
}
