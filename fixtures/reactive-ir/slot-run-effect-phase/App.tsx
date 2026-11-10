import { createEffect, createMemo, createSignal, flush, getOwner, isPending, latest, runWithOwner, untrack } from "solid-js";

// Positives. An effect's apply runs with no owner, and a callback that runs in place keeps that: a computation
// created in it, or `flush()`, meets the effect phase's guard (`PRIMITIVE_IN_EFFECT_CALLBACK`, `FLUSH_IN_EFFECT_CALLBACK`).
export function LatestMemo() {
  const [n] = createSignal(1);
  createEffect(() => n(), () => {
    latest(() => {
      createMemo(() => n());
    });
  });
  return <div />;
}

export function IsPendingMemo() {
  const [n] = createSignal(1);
  createEffect(() => n(), () => {
    isPending(() => {
      createMemo(() => n());
    });
  });
  return <div />;
}

// `getOwner()` in the apply is `null`, and so is the owner the callback runs under.
export function OwnerCapturedInTheApply() {
  const [n] = createSignal(1);
  createEffect(() => n(), () => {
    const owner = getOwner();
    runWithOwner(owner, () => {
      createMemo(() => n());
    });
  });
  return <div />;
}

export function NullOwner() {
  const [n] = createSignal(1);
  createEffect(() => n(), () => {
    runWithOwner(null, () => {
      createMemo(() => n());
    });
  });
  return <div />;
}

export function FlushCallbackMemo() {
  const [n] = createSignal(1);
  createEffect(() => n(), () => {
    flush(() => {
      createMemo(() => n());
    });
  });
  return <div />;
}

function runsDetached() {
  runWithOwner(null, () => {
    createMemo(() => 1);
  });
}

export function HelperWithANullOwner() {
  const [n] = createSignal(1);
  createEffect(() => n(), () => {
    runsDetached();
  });
  return <div />;
}

// `flush()` on the effect phase's stack drains nothing, whatever the callback it is written in.
export function LatestFlush() {
  const [n] = createSignal(1);
  createEffect(() => n(), () => {
    latest(() => {
      flush();
    });
  });
  return <div />;
}

export function IsPendingFlush() {
  const [n] = createSignal(1);
  createEffect(() => n(), () => {
    isPending(() => {
      flush();
    });
  });
  return <div />;
}

// Uncertifiable. A call written before may have run an effect, which lowers the marker the guard reads.
export function AnEarlierCall() {
  const [n] = createSignal(1);
  createEffect(() => n(), () => {
    console.log(1);
    untrack(() => {
      createMemo(() => n());
    });
  });
  return <div />;
}

// Negatives. An owner captured in the component body is the component's, and `getOwner()` is no call that may have
// run an effect: the capture before an `untrack` is the same finding as without it.
export function ComponentOwner() {
  const [n] = createSignal(1);
  const owner = getOwner();
  createEffect(() => n(), () => {
    runWithOwner(owner, () => {
      createMemo(() => n());
    });
  });
  return <div />;
}

export function OwnerReassigned() {
  const [n] = createSignal(1);
  const outer = getOwner();
  createEffect(() => n(), () => {
    let owner = getOwner();
    owner = outer;
    runWithOwner(owner, () => {
      createMemo(() => n());
    });
  });
  return <div />;
}

export function InTheCompute() {
  const [n] = createSignal(1);
  createEffect(() => {
    latest(() => {
      createMemo(() => n());
    });
    return n();
  }, () => {});
  return <div />;
}
