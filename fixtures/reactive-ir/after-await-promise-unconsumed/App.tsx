import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
const pause = () => Promise.resolve();
const peek = () => count();

// ADR 1090 (G1 residual): rc.21 raises UNTRACKED_READ_AFTER_AWAIT only for a read whose V8 async stack holds the frame that awaits an async computation's returned promise
// (`attributedFlight`). The async stack is the chain of code awaiting the current promise, so a function whose promise nothing consumes cannot carry that frame.

// Clean: a named async function every reference of which is a discarded direct call.
const fire = async () => {
  await pause();
  peek();
};
fire();

// Clean: a handler of a discarded standard `Promise.then` on a native promise.
Promise.resolve().then(() => {
  peek();
});

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
