import { createEffect, createMemo, createRenderEffect, createSignal } from "solid-js";

// An effect apply's read of an async source: rc.17 throws
// PENDING_ASYNC_UNTRACKED_READ when the run meets the source pending
// (`ApplyReadsAsync`, `RenderApplyReadsAsync`, `RenderApplyReadsAsyncRendered`)
// and warns STRICT_READ_UNTRACKED when it meets it settled (`ApplyReadsAsyncRendered`,
// whose mount waits on the rendered read; `ApplyComputeReadsAsync`, whose compute
// waits; `ApplyComputeWaitsOnOtherAsync`). The strict read is reported in each;
// the async rule claims none.

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
