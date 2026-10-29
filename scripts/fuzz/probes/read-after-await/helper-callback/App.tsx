// ADR 0485: a signal read in a callback a helper calls synchronously, after the memo's await, is untracked (UNTRACKED_READ_AFTER_AWAIT).
import { createEffect, createMemo, createSignal } from "solid-js";

function run(cb: () => unknown) {
  cb();
  return 1;
}

export default function App() {
  const [count] = createSignal(0);
  const m = createMemo(async () => {
    await Promise.resolve();
    run(() => count());
    return 1;
  });
  createEffect(() => m(), () => {});
  return <span id="w">{count()}</span>;
}
