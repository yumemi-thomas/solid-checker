import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
const pause = () => Promise.resolve();
const peek = () => count();

// ADR 1090 (G1 residual): rc.21 raises UNTRACKED_READ_AFTER_AWAIT only for a read whose V8 async stack holds the frame that awaits an async computation's returned promise
// (`attributedFlight`). The async stack is the chain of code awaiting the current promise, so a function whose promise nothing consumes cannot carry that frame.

// Clean: a named async function every reference of which is a discarded direct call, whose site is a direct read of an exact accessor.
const fire = async () => {
  await pause();
  count();
};
fire();

// Uncertifiable (review F1 of be4990c64): a `.then` handler. Its receiver is a native promise only when no `Promise` subclass or patch stands
// in, and no exact fact says so: a subclass whose constructor keeps `this` hands the derived promise to whatever awaits it.
Promise.resolve().then(() => {
  peek();
});
class Kept extends Promise<void> {
  static last?: Promise<unknown>;
  constructor(executor: (resolve: (value: void) => void, reject: (reason?: unknown) => void) => void) {
    super(executor);
    Kept.last = this;
  }
}
Kept.resolve().then(async () => {
  await pause();
  peek();
});

// Uncertifiable (review F2): the loop's first pass reads before the first await, on the caller's stack, inside whatever computation calls it.
const looped = async () => {
  for (let i = 0; i < 2; i++) {
    peek();
    await pause();
  }
};
looped();

// Uncertifiable (second review of 1090 G1, G1): the read is a class's instance field initializer, which runs wherever `new` runs (here a
// `toString` an async memo's template literal may run after its own await), not in the function that wrote the class.
const classBox: { K?: new () => unknown } = {};
const classed = async () => {
  await pause();
  classBox.K = class {
    x = peek();
  };
};
classed();

// Uncertifiable (third review, H1): the site is a project call whose result is a class; the class's field reads wherever `new` runs.
const makeClass = () =>
  class {
    x = peek();
  };
const built = async () => {
  await pause();
  classBox.K = makeClass();
};
built();

// Uncertifiable (third review, H3): a read in a JSX attribute compiles to a getter, read wherever the prop is read.
function Shown(props: { v: unknown }) {
  return <p>{String(props.v)}</p>;
}
let keptView: unknown;
const jsxed = async () => {
  await pause();
  keptView = <Shown v={count()} />;
};
jsxed();
export const view = () => keptView;

// Uncertifiable (fail closed): the promise is consumed by `void`, a unary whose operand is the promise.
const voided = async () => {
  await pause();
  peek();
};
void voided();

// Uncertifiable: the promise is stored.
const stored = async () => {
  await pause();
  peek();
};
export const kept = stored();

// Uncertifiable: an export a host may call and await, referenced or not.
export async function exported() {
  await pause();
  peek();
}

// Uncertifiable: the `.then` derived promise is returned to whoever awaits it.
export function chained() {
  return Promise.resolve().then(() => {
    peek();
  });
}

// Not ended by this reason: an async computation awaits the function, so its frame is on the read's async stack (Chrome: UNTRACKED_READ_AFTER_AWAIT at the read).
const awaited = async () => {
  await pause();
  return count();
};
export const total = createMemo(async () => {
  return await awaited();
});
