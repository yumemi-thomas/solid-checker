import { createEffect, createRenderEffect, createSignal } from "solid-js";

// The subject: a callback forwarded into `createRenderEffect`'s apply. The
// plain two-argument call runs the first apply before `createRenderEffect`
// returns (`@solidjs/signals@2.0.0-rc.3` `dist/dev.js:5285-5289`), so
// `deferred` would be false. `inline` would be false too: the same call runs
// the apply later when the compute returns a promise or reads a pending
// source, and every later run comes from the flush. No execution word is
// true, so the export's `callbacks` domain stays open.
export function renderApplyWrapper(handle: (value: number) => void): void {
  createRenderEffect(
    () => 1,
    value => handle(value)
  );
}

// The same slot with the parameter handed over directly rather than called
// from an arrow: the primitive's own slot answer is what refuses here.
export function renderApplyForwarded(handle: (value: number) => void): void {
  createRenderEffect(() => 1, handle);
}

// Negative: `createEffect` queues its apply on every run, so `deferred` holds.
export function effectApplyWrapper(handle: (value: number) => void): void {
  createEffect(
    () => 1,
    value => handle(value)
  );
}

// Negative: the render effect's compute is tracked and runs during the call,
// exactly as before; only the apply slot changed.
export function renderComputeWrapper(handle: () => number): void {
  createRenderEffect(
    () => handle(),
    () => {}
  );
}

// Reads. The first apply runs during the call with the caller's listener
// current, so `shared()` there is a read the export performs: a caller
// inside a memo subscribes to it. It used to be dropped, and the `reads`
// domain closed over nothing.
const [shared] = createSignal(0);
export function renderApplyRead(): void {
  createRenderEffect(
    () => 1,
    () => {
      shared();
    }
  );
}

// Negative: a `createEffect` apply reads after the call returns, so the
// export reads nothing during its call.
export function effectApplyRead(): void {
  createEffect(
    () => 1,
    () => {
      shared();
    }
  );
}
