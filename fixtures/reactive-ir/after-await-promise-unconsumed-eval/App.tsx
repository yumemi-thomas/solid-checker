import { createEffect, createMemo } from "solid-js";

const pause = () => new Promise((resolve) => setTimeout(resolve, 0));
// A memo accessor, not a signal tuple: a direct `eval` withdraws a tuple's slots (SC9012 `tuple-iteration-written`), which would hide this site behind a row of its own.
const source = createMemo(() => 1);

// Uncertifiable (review F3 of be4990c64): every reference of `fire` written in code is a discarded call, but a direct `eval` may call it where its
// promise is consumed. Here the string's call runs in the memo's compute, which returns the promise, so the read after the await is the memo's
// (Chrome rc.21: UNTRACKED_READ_AFTER_AWAIT at the read; be4990c64 kept only SC1001's row there).
const fire = async () => {
  await pause();
  source();
};
fire();

export function P1() {
  const run = createMemo(() => eval("fire()"));
  createEffect(() => run(), () => {});
  return <p>p</p>;
}
