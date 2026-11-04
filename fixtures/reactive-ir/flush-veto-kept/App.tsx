import { createEffect, createSignal, flush, runWithOwner } from "solid-js";

// Every flush of this program runs with no owner, or drains nothing, so the
// apply's ownerless proofs stand.

// Module evaluation: no owner is current.
flush();

// A computation created on the apply's stack: SC6002 and SC4001.
export function CreatesInApply() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      createEffect(
        () => 1,
        () => {},
      );
    },
  );
  return <div />;
}

// A write in the apply: legal, so silent.
export function WritesInApply() {
  const [count] = createSignal(0);
  const [seen, setSeen] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      setSeen(1);
    },
  );
  return <div>{seen()}</div>;
}

export function Drains() {
  const [count, setCount] = createSignal(0);
  // A flush in an apply: the queue is running there, so it drains nothing
  // (SC2008 reports it).
  createEffect(
    () => count(),
    () => {
      flush();
    },
  );
  // A host timer's callback runs on an empty stack.
  setTimeout(() => {
    flush();
  }, 0);
  // runWithOwner(null, …) runs its callback with no owner.
  runWithOwner(null, () => {
    flush();
  });
  // An element's event handler runs from the delivery.
  return (
    <button
      onClick={() => {
        setCount(1);
        flush();
      }}
    >
      {count()}
    </button>
  );
}

// Past an await its function reaches on every path, the call resumes from a
// microtask with no owner.
export async function later(wait: Promise<void>) {
  await wait;
  flush();
}

// A write in the cleanup the apply returns: a rerun runs it from the drain.
export function WritesInReturnedCleanup() {
  const [count] = createSignal(0);
  const [seen, setSeen] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      return () => {
        setSeen(1);
      };
    },
  );
  return <div>{seen()}</div>;
}
