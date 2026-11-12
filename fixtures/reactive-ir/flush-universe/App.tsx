import { action, createEffect, createSignal, createTrackedEffect, flush } from "solid-js";

const [n, setN] = createSignal(0);

// Batch 11, slice 5: every flush the flush census lists ends, per code (SC2006 an action's step, SC2008 an effect's apply, SC3001 a leaf owner's
// callback), in a row of that code's rule, an SC9012, the compiler's discard, a typed reason, or an uncertifiable row of the code's own rule. This program steps
// an action, runs an effect's apply and opens a leaf scope, so no program-wide reason holds: each flush stands on the census class of its own caller.

export const step = action(function* () {
  setN(1);
  yield 1;
});

export function Effect() {
  createEffect(() => n(), () => {});
  return <div />;
}

export function Leaf() {
  createTrackedEffect(() => {
    n();
  });
  return <div />;
}

// An uncertifiable row of each of the three codes: the helper's every invocation is an event delivery (the census's `EventHandler`), which rules no code
// out (a delivery may be dispatched from a step), and no producer rowed the flush (the walks answer "placed off", which is not a proof, owner ruling 5a).
function save() {
  flush();
}

export function Button() {
  return <button onClick={() => save()}>go</button>;
}

// Silent: a callback a reviewed scheduler runs from an empty stack, written directly in it (`FreshStack`).
setTimeout(() => {
  flush();
}, 10);

// Silent: past an `await` its own async function reaches on every path (`AwaitContinuation`).
export async function later() {
  await Promise.resolve();
  flush();
}

// Silent: module evaluation (`ModuleInitialization`).
flush();

// Silent: a function nothing enters (`NeverRuns`).
function unused() {
  flush();
}
