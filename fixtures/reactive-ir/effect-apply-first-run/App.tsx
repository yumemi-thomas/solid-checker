import { createEffect, createMemo } from "solid-js";

// ADR 0505: a user effect's first apply runs in the tail flush of the
// module-scope `render` that mounts this component (main.tsx), before the
// async source settles; nothing in the mount or before the render can hold
// that flush, so the read throws PENDING_ASYNC_UNTRACKED_READ.

// Violation (SC5001): the apply reads the pending source on its first run.
export function Proven() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  return (
    <div>
      <p>static</p>
    </div>
  );
}

// Uncertifiable (SC1001 stays): its render is not the first one, and an
// earlier mount's pending pass could hold the flush.
export function RenderedSecond() {
  const value = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      value();
    },
  );
  return <div />;
}
