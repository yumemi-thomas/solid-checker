import {
  action, createEffect, createMemo, createRoot, createSignal, createTrackedEffect, flush, getOwner, isPending, latest,
  onCleanup, onSettled, resolve, runWithOwner, untrack,
} from "solid-js";

// Positives. A read after an `await` in an async computation is untracked; a callback of `latest`, `isPending`,
// `flush(fn)` or `runWithOwner(null, fn)` runs in place on that continuation, so a read in it is too
// (UNTRACKED_READ_AFTER_AWAIT, `@solidjs/signals@2.0.0-rc.21`).
export function LatestRead() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    await Promise.resolve();
    latest(() => {
      n();
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

export function IsPendingRead() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    await Promise.resolve();
    isPending(() => {
      n();
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

export function FlushRead() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    await Promise.resolve();
    flush(() => {
      n();
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

export function NullOwnerRead() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    await Promise.resolve();
    runWithOwner(null, () => {
      n();
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

export function LatestInLatest() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    await Promise.resolve();
    latest(() => {
      latest(() => {
        n();
      });
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

function readsIt() {}

// Negatives. `untrack` is the documented way to read here on purpose; `createRoot`'s callback has an owner; a read
// before the `await` is tracked; a listener's callback runs later.
export function UntrackRead() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    await Promise.resolve();
    untrack(() => {
      n();
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

export function RootRead() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    await Promise.resolve();
    createRoot(() => {
      n();
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

export function UntrackedLatest() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    await Promise.resolve();
    untrack(() => {
      latest(() => {
        n();
      });
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

export function BeforeTheAwait() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    latest(() => {
      n();
    });
    await Promise.resolve();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Uncertifiable. The owner is a value captured before the `await`: `runWithOwner` silences the read for a real owner.
export function CapturedOwnerRead() {
  const [n] = createSignal(1);
  const m = createMemo(async () => {
    const owner = getOwner();
    await Promise.resolve();
    runWithOwner(owner, () => {
      n();
    });
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
