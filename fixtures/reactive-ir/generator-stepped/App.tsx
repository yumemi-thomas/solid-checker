import { createMemo, createSignal, createStore } from "solid-js";

const [a] = createSignal(1);
const [b] = createSignal(2);
const [d] = createSignal(4);
const [e] = createSignal(5);
const [f] = createSignal(6);
const [g] = createSignal(7);
const [h] = createSignal(8);
const [st] = createStore({ v: 3 });

// A `for await` steps the generator from the computation's continuation: past
// its first `await` the read is proved, as an `async` helper's is.
async function* afterAwait() {
  await Promise.resolve();
  yield a();
  yield st.v;
}
const proved = createMemo(async () => {
  let last = 0;
  for await (const value of afterAwait()) last = value;
  return last;
});

// Past a `yield`: open, the next step decides where it runs.
async function* afterYield() {
  yield 1;
  yield b();
}
const open = createMemo(async () => {
  let last = 0;
  for await (const value of afterYield()) last = value;
  return last;
});

// Any other stepping is open: a wrapper's iterator, `next()` by hand.
async function* wrapped() {
  await Promise.resolve();
  yield d();
}
const wrap = () => wrapped();
const through = createMemo(async () => {
  let last = 0;
  for await (const value of wrap()) last = value;
  return last;
});
async function* stepped() {
  await Promise.resolve();
  yield e();
}
const byHand = createMemo(async () => {
  const step = await stepped().next();
  return step.value ?? 0;
});

// A sync generator stepped across the loop's `await`.
function* acrossAwait() {
  yield 1;
  yield f();
}
const loop = createMemo(async () => {
  let last = 0;
  for (const value of acrossAwait()) {
    await Promise.resolve();
    last = value;
  }
  return last;
});

// Negative: the first segment runs on the computation's stack, a tracked read.
async function* firstSegment() {
  yield g();
}
const tracked = createMemo(async () => {
  let last = 0;
  for await (const value of firstSegment()) last = value;
  return last;
});

// Negative: the loop drains a sync generator before the only `await`.
function* drained() {
  yield 1;
  yield h();
}
const quiet = createMemo(async () => {
  let last = 0;
  for (const value of drained()) last = value;
  await Promise.resolve();
  return last;
});

export const App = () => <p title={String(proved() + open() + through() + byHand() + loop() + tracked() + quiet())} />;
