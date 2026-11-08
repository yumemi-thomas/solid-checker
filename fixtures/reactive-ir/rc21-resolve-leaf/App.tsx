import { createMemo, createSignal, createTrackedEffect, onSettled, resolve, until } from "solid-js";

const [count] = createSignal(1);

function helper() {
  return resolve(() => count());
}

function waits() {
  return until(() => count());
}

export function Tracked() {
  createTrackedEffect(() => {
    helper();
    waits();
  });
  return <p>{1}</p>;
}

export function Direct() {
  createTrackedEffect(() => {
    void resolve(() => count());
  });
  return <p>{1}</p>;
}

export function Owned() {
  createTrackedEffect(() => {
    createMemo(() => 1);
  });
  return <button onClick={() => helper()}>x</button>;
}

// `onSettled` runs under `untrack`, with no observer: the same call throws the
// leaf owner's code, so it stays that rule's.
export function Settled() {
  onSettled(() => {
    void resolve(() => 1);
  });
  return <p>{1}</p>;
}
