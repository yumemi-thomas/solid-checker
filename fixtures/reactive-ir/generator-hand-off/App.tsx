import { createMemo, createSignal } from "solid-js";

const [a] = createSignal(1);
const [b] = createSignal(2);
const [c] = createSignal(3);
const [d] = createSignal(4);
const [e] = createSignal(5);
const [f] = createSignal(6);
const [g] = createSignal(7);

// Past an async generator's `yield`, stepped by `for await`: a function the
// body hands on and then runs is an obligation, as a direct call is.
const member = createMemo(async () => {
  async function* gen() {
    yield 1;
    const hf = () => {
      a();
    };
    const ho = { m: hf };
    ho.m();
  }
  for await (const _y of gen()) void _y;
  return 1;
});

// An assigned member.
const assigned = createMemo(async () => {
  async function* gen() {
    yield 1;
    const ho: { m?: () => void } = {};
    ho.m = () => {
      b();
    };
    ho.m?.();
  }
  for await (const _y of gen()) void _y;
  return 1;
});

// A member a helper assigns through its parameter.
const filled = createMemo(async () => {
  async function* gen() {
    yield 1;
    const fill = (o: { m?: () => void }) => {
      o.m = () => {
        c();
      };
    };
    const ho: { m?: () => void } = {};
    fill(ho);
    ho.m?.();
  }
  for await (const _y of gen()) void _y;
  return 1;
});

// A callback forwarded through two helpers.
const forwarded = createMemo(async () => {
  async function* gen() {
    yield 1;
    const hc0 = (k: () => void) => { k(); };
    const hc1 = (k: () => void) => { hc0(k); };
    hc1(() => {
      d();
    });
  }
  for await (const _y of gen()) void _y;
  return 1;
});

// A sync generator a loop steps across its `await`: the same hand-offs.
const syncMember = createMemo(async () => {
  function* gen() {
    yield 1;
    const hf = () => {
      e();
    };
    const ho = { m: hf };
    ho.m();
  }
  for (const _y of gen()) await Promise.resolve();
  return 1;
});

const syncForwarded = createMemo(async () => {
  function* gen() {
    yield 1;
    const hc0 = (k: () => void) => { k(); };
    const hc1 = (k: () => void) => { hc0(k); };
    hc1(() => {
      f();
    });
  }
  for (const _y of gen()) await Promise.resolve();
  return 1;
});

// Negative: the same hand-off in a sync generator the loop drains before the
// only `await` runs on the computation's stack, a tracked read.
const drained = createMemo(async () => {
  function* gen() {
    yield 1;
    const hf = () => {
      g();
    };
    const ho = { m: hf };
    ho.m();
  }
  for (const _y of gen()) void _y;
  await Promise.resolve();
  return 1;
});

// Negative: the twins with the read replaced by a plain value read nothing.
const plain = { v: 9 };
const twin = createMemo(async () => {
  async function* gen() {
    yield 1;
    const hf = () => {
      void plain.v;
    };
    const ho = { m: hf };
    ho.m();
  }
  for await (const _y of gen()) void _y;
  return 1;
});
const syncTwin = createMemo(async () => {
  function* gen() {
    yield 1;
    const hc0 = (k: () => void) => { k(); };
    const hc1 = (k: () => void) => { hc0(k); };
    hc1(() => {
      void plain.v;
    });
  }
  for (const _y of gen()) await Promise.resolve();
  return 1;
});

export const App = () => (
  <p title={String(member() + assigned() + filled() + forwarded() + syncMember() + syncForwarded() + drained() + twin() + syncTwin())} />
);
