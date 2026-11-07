import { createMemo, createSignal, onCleanup } from "solid-js";

const [, setA] = createSignal(0);
const [, setB] = createSignal(0);
const [, setC] = createSignal(0);
const [, setD] = createSignal(0);
const [, setE] = createSignal(0);
const [, setF] = createSignal(0);

// Past a `yield` of an async generator `for await` steps: the step runs on the
// compute's continuation, with no owner and no observer. The write is legal
// there; `onCleanup` has no owner.
const awaited = createMemo(async () => {
  async function* gen() {
    yield 1;
    setA(1);
    onCleanup(() => {});
  }
  for await (const _y of gen()) void _y;
  return 1;
});

// A sync generator a loop steps across the `await` its body starts with.
const looped = createMemo(async () => {
  function* gen() {
    yield 1;
    setB(1);
    onCleanup(() => {});
  }
  for (const _y of gen()) await Promise.resolve();
  return 1;
});

// The loop's `await` is not first in its body: whether the next step follows
// it is not proven, so both answers are open.
const later = createMemo(async () => {
  function* gen() {
    yield 1;
    setC(1);
    onCleanup(() => {});
  }
  for (const y of gen()) {
    if (y > 1) continue;
    await Promise.resolve();
  }
  return 1;
});

// Negative: the loop drains the generator before the only `await`, on the
// compute's stack: the write is in the compute, and the cleanup is owned.
const drained = createMemo(async () => {
  function* gen() {
    yield 1;
    setD(1);
    onCleanup(() => {});
  }
  for (const _y of gen()) void _y;
  await Promise.resolve();
  return 1;
});

// Negative: the first segment runs on the compute's stack.
const first = createMemo(async () => {
  async function* gen() {
    setE(1);
    yield 1;
  }
  for await (const _y of gen()) void _y;
  return 1;
});

// Negative: the generator has another use, so this stepping is not the only
// one that runs it.
const shared = createMemo(async () => {
  async function* gen() {
    yield 1;
    setF(1);
  }
  const again = gen;
  for await (const _y of gen()) void _y;
  return again === gen ? 1 : 0;
});

export const App = () => (
  <p title={String(awaited() + looped() + later() + drained() + first() + shared())} />
);
