import { createEffect, createMemo, createRenderEffect, createSignal } from "solid-js";

// An effect apply's read of an async source: rc.17 throws
// PENDING_ASYNC_UNTRACKED_READ when the run meets the source pending
// (`ApplyReadsAsync`, `RenderApplyReadsAsync`, `RenderApplyReadsAsyncRendered`)
// and warns STRICT_READ_UNTRACKED when it meets it settled (`ApplyReadsAsyncRendered`,
// whose mount waits on the rendered read; `ApplyComputeReadsAsync`, whose compute
// waits; `ApplyComputeWaitsOnOtherAsync`). Where the first run is proven to
// meet the source pending (ADRs 0504, 0505; README) the async rule claims the
// read; elsewhere the strict read is reported.

export function ApplyReadsAsync() {
  const data = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      data();
    }
  );
  return <p>x</p>;
}

export function RenderApplyReadsAsync() {
  const data = createMemo(async () => 1);
  createRenderEffect(
    () => 1,
    () => {
      data();
    }
  );
  return <p>x</p>;
}

export function RenderApplyReadsAsyncRendered() {
  const data = createMemo(async () => 1);
  createRenderEffect(
    () => 1,
    () => {
      data();
    }
  );
  return <p>{data()}</p>;
}

export function ApplyReadsAsyncRendered() {
  const data = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      data();
    }
  );
  return <p>{data()}</p>;
}

export function ApplyComputeReadsAsync() {
  const data = createMemo(async () => 1);
  createEffect(
    () => data(),
    () => {
      data();
    }
  );
  return <p>x</p>;
}

export function ApplyComputeWaitsOnOtherAsync() {
  const slow = createMemo(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
    return 2;
  });
  const data = createMemo(async () => 1);
  createEffect(
    () => slow(),
    () => {
      data();
    }
  );
  return <p>x</p>;
}

// Controls: a sync source in the apply (STRICT_READ_UNTRACKED, reported) and
// an async source read in the body (PENDING_ASYNC_UNTRACKED_READ: the async
// rule, with no strict read beside it).

export function ApplyReadsSync() {
  const [count] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      count();
    }
  );
  return <p>x</p>;
}

export function BodyReadsAsync() {
  const data = createMemo(async () => 1);
  data();
  return <p>x</p>;
}

// ADR 0504: the same apply as `ApplyReadsAsync`, but no module-scope render
// mounts it (main.tsx), so the first run's timing is unproven: the strict read.
export function UnmountedApplyReadsAsync() {
  const data = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      data();
    }
  );
  return <p>x</p>;
}

// ADR 0504 review negatives (mounted; the strict read stays): a closure two
// effects call, a call in the apply before the read (it may throw first), and
// a read in the body before the effect is created (the body read throws).
export function SharedClosure() {
  const data = createMemo(async () => 1);
  const show = () => {
    data();
  };
  createRenderEffect(
    () => 1,
    () => {
      show();
    }
  );
  createEffect(
    () => 1,
    () => {
      show();
    }
  );
  return <p>x</p>;
}

export function ReadAfterCall() {
  const data = createMemo(async () => 1);
  createRenderEffect(
    () => 1,
    () => {
      console.log("first");
      data();
    }
  );
  return <p>x</p>;
}

export function BodyReadsFirst() {
  const data = createMemo(async () => 1);
  data();
  createRenderEffect(
    () => 1,
    () => {
      data();
    }
  );
  return <p>x</p>;
}

// ADR 0504 review: the render effect's first run returns before the read
// (its value is the compute's 1), so it never meets the source pending: the
// strict read stays the claim.
export function RenderApplyGuarded() {
  const data = createMemo(async () => 1);
  createRenderEffect(
    () => 1,
    (v) => {
      if (v === 1) return;
      data();
    }
  );
  return <p>x</p>;
}

// ADR 0504 review: the same guard in a closure the apply calls.
export function RenderApplyGuardedClosure() {
  const data = createMemo(async () => 1);
  createRenderEffect(
    () => 1,
    (v) => {
      const step = () => {
        if (v === 1) return;
        data();
      };
      step();
    }
  );
  return <p>x</p>;
}
