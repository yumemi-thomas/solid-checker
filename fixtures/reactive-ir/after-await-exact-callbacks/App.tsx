import { createMemo, createSignal } from "solid-js";

// Violations (SC1002), each confirmed by rc.17 in Chrome
// (UNTRACKED_READ_AFTER_AWAIT): the async compute is named exactly (ADR 0517).
export function NamedExpression() {
  const [r] = createSignal(3);
  const compute = async function named() {
    await Promise.resolve();
    r();
    return 1;
  };
  const m = createMemo(compute);
  return <p>{m()}</p>;
}

export function ObjectSlot() {
  const [r] = createSignal(3);
  const box = { compute: async () => {
    await Promise.resolve();
    r();
    return 1;
  } };
  const m = createMemo(box.compute);
  return <p>{m()}</p>;
}

export function AssignedCallee() {
  const [r] = createSignal(3);
  let slot: typeof r | undefined;
  const m = createMemo(async () => {
    await Promise.resolve();
    (slot = r)();
    return 1;
  });
  return <p>{m()}</p>;
}

// Uncertifiable: `slot ||= r` yields `slot` once it is set.
export function OrAssignedCallee() {
  const [r] = createSignal(3);
  let slot: typeof r | undefined;
  const m = createMemo(async () => {
    await Promise.resolve();
    (slot ||= r)();
    return 1;
  });
  return <p>{m()}</p>;
}

// Clean: the read runs before the first await and is tracked.
export function ReadBeforeAwait() {
  const [r] = createSignal(3);
  const compute = async function named() {
    r();
    await Promise.resolve();
    return 1;
  };
  const m = createMemo(compute);
  return <p>{m()}</p>;
}

// Uncertifiable (rc.17 in Chrome: UNTRACKED_READ_AFTER_AWAIT, recall grid): a
// read in a `for await` body of a compute named by an object slot, and an
// assignment callee on a later pass of a loop that awaits or in a `finally`
// after an awaited `try` head (ADR 0517).
export function ObjectSlotForAwait() {
  const [r] = createSignal(3);
  const box = { compute: async () => {
    for await (const _x of [1]) {
      r();
    }
    return 1;
  } };
  const m = createMemo(box.compute);
  return <p>{m()}</p>;
}

export function AssignedCalleeLoop() {
  const [r] = createSignal(3);
  let slot: typeof r | undefined;
  const m = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        (slot = r)();
      }
      await Promise.resolve();
    }
    return 1;
  });
  return <p>{m()}</p>;
}

export function AssignedCalleeFinally() {
  const [r] = createSignal(3);
  let slot: typeof r | undefined;
  const m = createMemo(async () => {
    try { await Promise.resolve(); } finally {
      (slot = r)();
    }
    return 1;
  });
  return <p>{m()}</p>;
}

// Uncertifiable: the read in a closure that body runs, and through a wrapper.
export function ObjectSlotForAwaitLayers() {
  const [r] = createSignal(3);
  const wrapped = () => r();
  const box = { compute: async () => {
    for await (const _x of [1]) {
      const read = () => { r(); };
      read();
      wrapped();
    }
    return 1;
  } };
  const m = createMemo(box.compute);
  return <p>{m()}</p>;
}
