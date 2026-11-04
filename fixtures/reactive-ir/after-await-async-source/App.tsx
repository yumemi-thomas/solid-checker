import { createEffect, createMemo, createSignal } from "solid-js";

// An async source read past an await throws pending and warns settled: not proven either way.
export function Pending() {
  const [t] = createSignal(1);
  const a = createMemo(async () => {
    t();
    await Promise.resolve();
    return 1;
  });
  const m = createMemo(async () => {
    await Promise.resolve();
    a(); // SC1002 uncertifiable
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
