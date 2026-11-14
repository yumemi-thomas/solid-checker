import { createMemo, createRoot, createSignal, createStore } from "solid-js";

const [source] = createSignal(1);
const [store] = createStore({ x: 1, n: { x: 1 } });
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973), the review of #233's B1: a call written after an await whose callee's closure lists no reactive read still runs one. The first build ended these
// calls with a typed reason ("the callee reads nothing"), silently, and Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT under each: a read reaches a continuation through values no
// call shows. There is no callee reason now; the call keeps SC1002's uncertifiable row (a frame of the stack the diagnostic names).

// A store read through an alias binding.
const inner = store.n;
function peekAlias() {
  return inner.x;
}

// A stored generator object stepped by a spread.
function* numbers() {
  yield source();
}
const stored = numbers();
function peekGenerator() {
  return [...stored];
}

// A Proxy trap and a getter installed through a handler and a descriptor declared apart (the inline spellings have a row of their own at the read).
function reader() {
  return source();
}
const traps = { get: reader };
const proxy: any = new Proxy({}, traps);
function peekProxy() {
  return proxy.x;
}
const descriptor = { get: reader };
const installed: any = {};
Object.defineProperty(installed, "value", descriptor);
function peekDescriptor() {
  return installed.value;
}

createRoot(() => {
  createMemo(async () => {
    await pause();
    return peekAlias();
  });
  createMemo(async () => {
    await pause();
    return peekGenerator();
  });
  createMemo(async () => {
    await pause();
    return peekProxy();
  });
  createMemo(async () => {
    await pause();
    return peekDescriptor();
  });
});

// A props alias: the parent passes a getter, the child keeps the object it reads through.
function Child(props: { n: { y: number } }) {
  const nested = props.n;
  const peek = () => nested.y;
  const data = createMemo(async () => {
    await pause();
    return peek();
  });
  return <p>{String(data())}</p>;
}

export function Probe() {
  const [n] = createSignal(1);
  return <Child n={{ get y() { return n(); } }} />;
}
